import {
	WORKBENCH_PROTOCOL_VERSION,
	type TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';

import type {
	WorkbenchAgentConfirmation,
	WorkbenchAgentConfirmationChoice,
	WorkbenchAgentConfirmations,
} from '../../workbench';
import type { ServerConnection } from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import { listBoundPresentationIds } from './listBoundPresentationIds';
import {
	ServerPresentationRowSource,
	type ServerPanelAgentAction,
	type ServerPanelAgentOutcome,
} from './ServerPresentationRowSource';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

type ConfirmationRow = Extract<
	ServerPanelAgentOutcome,
	{ readonly kind: 'confirmations' }
>['presentations'][number];
type ConfirmationSession = Pick<
	ServerWorkbenchSession,
	'ask' | 'onEnded' | 'snapshot' | 'subscribe'
>;

/**
 * The Server applies a choice under one lock and sends its hold before it answers, so a late answer
 * means the request was lost behind this connection's other requests or with the connection.
 */
const REPLY_MILLISECONDS = 10_000;

const NOTHING: ReadonlyMap<TerminalPresentationId, WorkbenchAgentConfirmation> = new Map();

/**
 * How the Server answers Agent confirmations for every bound Presentation of this session, and the
 * two requests that change it. Only a Session whose handshake lists `agent-confirmation` serves
 * them, so its composition creates this only there. The Server owns every value: a choice shows
 * here only as the Server's answer or its next row reports it, never as what was asked for.
 */
export class ServerAgentConfirmations implements WorkbenchAgentConfirmations {
	readonly #listeners = new Set<() => void>();
	readonly #rows = new Map<TerminalPresentationId, ConfirmationRow>();
	readonly #session: ConfirmationSession;
	#snapshot = NOTHING;
	readonly #source: ServerPresentationRowSource<ConfirmationRow>;
	readonly #stopSession: () => void;

	constructor(connection: Pick<ServerConnection, 'subscribe'>, session: ConfirmationSession) {
		this.#session = session;
		this.#source = new ServerPresentationRowSource<ConfirmationRow>(
			{
				action: (presentationIds) => ({ kind: 'confirmations', presentationIds }),
				// The most Presentations the Server answers in one `confirmations` request.
				limit: 64,
				rows: (outcome) => (outcome.kind === 'confirmations' ? outcome.presentations : null),
			},
			connection,
			session,
			(rows) => this.#receive(rows),
		);
		// A Presentation that left the Workbench has no stream to say so.
		this.#stopSession = session.subscribe((snapshot) => {
			const bound = new Set(listBoundPresentationIds(snapshot));
			let changed = false;
			for (const presentationId of this.#rows.keys()) {
				if (!bound.has(presentationId)) changed = this.#rows.delete(presentationId) || changed;
			}
			if (changed) this.#publish();
		});
	}

	readonly getSnapshot = (): ReadonlyMap<TerminalPresentationId, WorkbenchAgentConfirmation> =>
		this.#snapshot;

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	};

	choose(
		presentationId: TerminalPresentationId,
		choice: WorkbenchAgentConfirmationChoice,
	): Promise<boolean> {
		return this.#change(presentationId, (expectedBinding) => ({
			choice,
			expectedBinding,
			kind: 'confirmation-choose',
			presentationId,
		}));
	}

	decline(presentationId: TerminalPresentationId): Promise<boolean> {
		return this.#change(presentationId, (expectedBinding) => ({
			expectedBinding,
			kind: 'confirmation-decline',
			presentationId,
		}));
	}

	/** Opens again every stream the Server closed, as it does after replacing a lost Worker. */
	reopenClosed(): void {
		this.#source.reopenClosed();
	}

	dispose(): void {
		this.#stopSession();
		this.#source.dispose();
		this.#listeners.clear();
	}

	async #change(
		presentationId: TerminalPresentationId,
		action: (expectedBinding: NonNullable<ConfirmationRow['binding']>) => ServerPanelAgentAction,
	): Promise<boolean> {
		// The terminal the person saw the choice for; the Server refuses any other.
		const binding = this.#rows.get(presentationId)?.binding ?? null;
		if (binding === null) return false;
		const requestId = createWorkbenchRequestId();
		let answer: Awaited<ReturnType<ConfirmationSession['ask']>>;
		try {
			answer = await this.#session.ask(
				{
					action: action(binding),
					kind: 'workbench.panel-agent',
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId,
					sessionId: this.#session.snapshot.sessionId,
				},
				REPLY_MILLISECONDS,
			);
		} catch {
			return false;
		}
		if (
			answer.kind !== 'answered' ||
			answer.response.kind !== 'workbench.panel-agent.result' ||
			answer.response.requestId !== requestId ||
			answer.response.outcome.kind !== 'confirmation-chosen' ||
			answer.response.outcome.presentation.presentationId !== presentationId
		) {
			return false;
		}
		this.#receive([answer.response.outcome.presentation]);
		return true;
	}

	#receive(rows: readonly ConfirmationRow[]): void {
		let changed = false;
		for (const row of rows) {
			const prior = this.#rows.get(row.presentationId);
			if (row.confirmation === null) {
				changed = this.#rows.delete(row.presentationId) || changed;
			} else if (prior === undefined || !sameRow(prior, row)) {
				this.#rows.set(row.presentationId, row);
				changed = true;
			}
		}
		if (changed) this.#publish();
	}

	#publish(): void {
		const next = new Map<TerminalPresentationId, WorkbenchAgentConfirmation>();
		for (const [presentationId, { confirmation }] of this.#rows) {
			if (confirmation !== null) next.set(presentationId, confirmation);
		}
		this.#snapshot = next;
		for (const listener of this.#listeners) listener();
	}
}

function sameRow(left: ConfirmationRow, right: ConfirmationRow): boolean {
	const [a, b] = [left.confirmation, right.confirmation];
	if (a === null || b === null) return a === b;
	return (
		left.binding?.workerId === right.binding?.workerId &&
		left.binding?.terminalId === right.binding?.terminalId &&
		a.agent === b.agent &&
		a.choice === b.choice &&
		a.offer === b.offer &&
		a.wait === b.wait &&
		a.stopped === b.stopped &&
		a.pressed?.serial === b.pressed?.serial &&
		a.pressed?.kind === b.pressed?.kind &&
		a.pressed?.text === b.pressed?.text
	);
}
