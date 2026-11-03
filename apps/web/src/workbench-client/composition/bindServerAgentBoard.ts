import {
	showWorkbenchPresentation,
	type WorkbenchAgentBoard,
	type WorkbenchAgentBoardJump,
	type WorkbenchAgentBoardJumpFailure,
	type WorkbenchAgentBoardRow,
	type WorkbenchAgentBoardSnapshot,
	type WorkbenchProjectWorkspaceRuntime,
} from '@orchbee/workbench';

import type { AgentSessionSummary, ServerConnection } from '../server-connection';
import { getServerAgentSession, listServerAgentSessions } from '../server-workbench';

/** The TUI's bounds: a read round, a jump's Get, a jump in all, and the pause after a read. */
const READ_DEADLINE_MILLISECONDS = 3_000;
const GET_DEADLINE_MILLISECONDS = 3_000;
const JUMP_DEADLINE_MILLISECONDS = 8_000;
const READ_COOLDOWN_MILLISECONDS = 2_000;
const MAXIMUM_ROWS = 600;

interface Jump {
	readonly resolve: (jump: WorkbenchAgentBoardJump) => void;
	readonly serial: number;
	readonly sessionId: string;
}

type Request = { readonly kind: 'list' } | { readonly kind: 'get'; readonly serial: number };

/**
 * The Agent board's reads, held as the TUI holds them. An open board is one episode: it reads once
 * the two seconds since the last read have passed, and a close drops whatever its reads still owe.
 * One request is out at a time, and a jump waiting for it takes the place of a waiting read. A jump
 * reads its session again, then shows the first of the session's terminals this window holds, in
 * the shown Tab before the others and visible before hidden; landing closes the episode.
 */
export function bindServerAgentBoard({
	connection,
	runtime,
}: {
	readonly connection: Pick<ServerConnection, 'exchange'>;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}): { readonly board: WorkbenchAgentBoard; dispose(): void } {
	const listeners = new Set<() => void>();
	let snapshot: WorkbenchAgentBoardSnapshot = {
		jumping: false,
		notice: null,
		read: { kind: 'unread' },
		rows: [],
	};
	let disposed = false;
	let open = false;
	let episode = 0;
	let intent: 'jump' | 'refresh' | null = null;
	let inflight: Request | null = null;
	let jump: Jump | null = null;
	let jumpSerial = 0;
	let lastReadEnd: number | null = null;
	let rowsFromComplete = false;
	let wake: ReturnType<typeof setTimeout> | null = null;
	let expiry: ReturnType<typeof setTimeout> | null = null;

	const publish = (next: Partial<WorkbenchAgentBoardSnapshot>): void => {
		snapshot = { ...snapshot, ...next };
		for (const listener of listeners) listener();
	};
	const clearTimer = (timer: ReturnType<typeof setTimeout> | null): null => {
		if (timer !== null) clearTimeout(timer);
		return null;
	};
	/** Ends the current jump, if any, with `outcome`; a failure leaves its notice. */
	const settleJump = (
		outcome: WorkbenchAgentBoardJump,
		notice: WorkbenchAgentBoardJumpFailure | null = null,
	): void => {
		const ended = jump;
		jump = null;
		expiry = clearTimer(expiry);
		if (intent === 'jump') intent = null;
		if (ended === null && notice === null) return;
		ended?.resolve(outcome);
		publish({ jumping: false, ...(notice === null ? {} : { notice }) });
	};
	const endEpisode = (): void => {
		open = false;
		episode += 1;
		intent = null;
		inflight = null;
		wake = clearTimer(wake);
		settleJump('superseded');
		if (snapshot.notice !== null) publish({ notice: null });
	};

	const listed = (round: Awaited<ReturnType<typeof listServerAgentSessions>>): void => {
		if (round.kind === 'failed') {
			publish({ read: { kind: 'failed', reason: round.reason } });
			return;
		}
		// A cut-short walk never replaces the rows of a whole one.
		const replace = round.complete || !rowsFromComplete;
		if (replace) rowsFromComplete = round.complete;
		publish({
			notice: null,
			read: { complete: round.complete, kind: 'listed' },
			rows: replace ? projectRows(round.items) : snapshot.rows,
		});
	};

	const issueList = (): void => {
		const issuedIn = episode;
		inflight = { kind: 'list' };
		publish({ read: { kind: 'reading' } });
		void listServerAgentSessions({
			connection,
			deadline: performance.now() + READ_DEADLINE_MILLISECONDS,
		}).then((round) => {
			if (disposed || issuedIn !== episode || inflight?.kind !== 'list') return;
			inflight = null;
			// Every finished attempt starts the pause, a failed one too.
			lastReadEnd = performance.now();
			listed(round);
			pump();
		});
	};

	const shown = (session: Awaited<ReturnType<typeof getServerAgentSession>>): void => {
		switch (session.kind) {
			case 'not-found':
				settleJump('failed', 'gone');
				return;
			case 'failed':
				settleJump('failed', session.reason === 'denied' ? 'denied' : 'unavailable');
				return;
			case 'found':
				break;
		}
		if (session.lifecycle === 'ended') {
			settleJump('failed', 'ended');
			return;
		}
		if (session.navigable.length === 0) {
			settleJump('failed', 'no-placement');
			return;
		}
		// No candidate this window can show means the session's terminals have gone from it.
		if (showWorkbenchPresentation(runtime, session.navigable).kind !== 'shown') {
			settleJump('failed', 'gone');
			return;
		}
		settleJump('shown');
		endEpisode();
	};

	const issueGet = (current: Jump): void => {
		const issuedIn = episode;
		const { serial } = current;
		const now = performance.now();
		inflight = { kind: 'get', serial };
		expiry = clearTimer(expiry);
		expiry = setTimeout(() => {
			if (jump?.serial === serial) settleJump('failed', 'expired');
		}, JUMP_DEADLINE_MILLISECONDS);
		// The Get's own deadline falls inside the jump's.
		void getServerAgentSession(
			{ connection, deadline: now + GET_DEADLINE_MILLISECONDS },
			current.sessionId,
		).then((session) => {
			// The answer frees the request slot even when its jump has since been superseded.
			if (
				disposed ||
				issuedIn !== episode ||
				inflight?.kind !== 'get' ||
				inflight.serial !== serial
			) {
				return;
			}
			inflight = null;
			if (jump?.serial === serial) shown(session);
			pump();
		});
	};

	/** Sends what waits once nothing is out, holding a read until the pause has passed. */
	const pump = (): void => {
		if (disposed || !open || inflight !== null) return;
		if (intent === 'jump' && jump !== null) {
			intent = null;
			issueGet(jump);
			return;
		}
		if (intent !== 'refresh') return;
		const remaining =
			lastReadEnd === null ? 0 : lastReadEnd + READ_COOLDOWN_MILLISECONDS - performance.now();
		if (remaining > 0) {
			wake = clearTimer(wake);
			wake = setTimeout(() => {
				wake = null;
				pump();
			}, remaining);
			return;
		}
		intent = null;
		issueList();
	};

	const board: WorkbenchAgentBoard = {
		close: () => {
			if (!disposed && open) endEpisode();
		},
		getSnapshot: () => snapshot,
		jump: (sessionId) => {
			if (disposed || !open) return Promise.resolve<WorkbenchAgentBoardJump>('superseded');
			const row = snapshot.rows.find((candidate) => candidate.sessionId === sessionId);
			// A row that names no terminal is refused before any read, leaving a jump under way alone.
			if (row === undefined || row.presentationId === null) {
				publish({ notice: row === undefined ? 'gone' : 'no-placement' });
				return Promise.resolve<WorkbenchAgentBoardJump>('failed');
			}
			settleJump('superseded');
			jumpSerial += 1;
			const serial = jumpSerial;
			return new Promise<WorkbenchAgentBoardJump>((resolve) => {
				jump = { resolve, serial, sessionId };
				intent = 'jump';
				publish({ jumping: true, notice: null });
				pump();
			});
		},
		open: () => {
			if (disposed) return;
			endEpisode();
			open = true;
			intent = 'refresh';
			pump();
		},
		refresh: () => {
			if (disposed || !open || intent === 'jump') return;
			if (lastReadEnd !== null && performance.now() < lastReadEnd + READ_COOLDOWN_MILLISECONDS) {
				return;
			}
			intent = 'refresh';
			publish({ notice: null });
			pump();
		},
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};

	return {
		board,
		dispose: () => {
			if (disposed) return;
			endEpisode();
			disposed = true;
			listeners.clear();
		},
	};
}

/** The Server's order, with the sessions that wait for the person lifted first. */
function projectRows(items: readonly AgentSessionSummary[]): readonly WorkbenchAgentBoardRow[] {
	const rows = items.map(projectRow);
	const waiting = rows.filter(({ status }) => status.kind === 'needs-attention');
	const others = rows.filter(({ status }) => status.kind !== 'needs-attention');
	return [...waiting, ...others].slice(0, MAXIMUM_ROWS);
}

function projectRow(summary: AgentSessionSummary): WorkbenchAgentBoardRow {
	const { status } = summary;
	return {
		ended: summary.lifecycle === 'ended',
		freshness: summary.observation.freshness,
		presentationId:
			summary.presentationAttachments.find(({ navigable }) => navigable)?.presentationId ?? null,
		providerId: summary.providerId,
		sessionId: summary.sessionId,
		status:
			status.kind === 'needs-attention'
				? { count: status.count, kind: 'needs-attention' }
				: { kind: status.kind },
	};
}
