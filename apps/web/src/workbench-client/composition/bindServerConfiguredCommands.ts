import {
	readWorkbenchShownPresentationId,
	showWorkbenchClientTarget,
	type WorkbenchConfiguredCommandOutcome,
	type WorkbenchConfiguredCommands,
	type WorkbenchProjectWorkspaceRuntime,
} from '../../workbench';

import type { ServerConnection } from '../server-connection';
import {
	ServerConfiguredCommands,
	findServerTerminalBinding,
	type ServerClientViewSample,
	type ServerConfiguredCommandReply,
	type ServerConfiguredCommandSource,
	type ServerWorkbenchSession,
} from '../server-workbench';
import type { BoundServerClientView } from './bindServerClientView';

/** How long a new Pane waits for the publication that holds it before the window gives up on it. */
const PUBLICATION_WAIT_MILLISECONDS = 5_000;

type Refusal = Extract<WorkbenchConfiguredCommandOutcome, { readonly kind: 'refused' }>['reason'];
type Answered = Extract<ServerConfiguredCommandReply, { readonly kind: 'answered' }>['outcome'];
type PaneOutcome = Extract<Answered, { readonly kind: 'pane' }>['result']['outcome'];

const ACCEPTED: WorkbenchConfiguredCommandOutcome = { kind: 'accepted' };
const FAILED: WorkbenchConfiguredCommandOutcome = { kind: 'failed' };
const UNKNOWN: WorkbenchConfiguredCommandOutcome = { kind: 'unknown' };

function refused(reason: Refusal): WorkbenchConfiguredCommandOutcome {
	return { kind: 'refused', reason };
}

/**
 * Offers the commands the Session's configuration installs and runs them as the TUI does: one at a
 * time, from the Panel the window shows, bound to the Server's record of the view that showed it. A
 * Pane a command opens takes the focus once the window holds it. A popup command is not offered,
 * since this window opens no popup terminal yet.
 */
export function bindServerConfiguredCommands({
	clientView,
	connection,
	runtime,
	session,
}: {
	readonly clientView: BoundServerClientView | null;
	readonly connection: Pick<ServerConnection, 'exchange' | 'subscribe'>;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
	readonly session: Pick<ServerWorkbenchSession, 'published' | 'snapshot'>;
}): { readonly commands: WorkbenchConfiguredCommands; dispose(): void } {
	const configured = new ServerConfiguredCommands({
		connection,
		sessionId: session.snapshot.sessionId,
	});
	let disposed = false;
	let running = false;
	const timers = new Set<ReturnType<typeof setTimeout>>();
	const wait = (milliseconds: number): Promise<void> =>
		new Promise((resolve) => {
			const timer = setTimeout(() => {
				timers.delete(timer);
				resolve();
			}, milliseconds);
			timers.add(timer);
		});

	let listed = configured.getSnapshot();
	let offered: ReturnType<WorkbenchConfiguredCommands['getSnapshot']> = [];
	const getSnapshot = (): ReturnType<WorkbenchConfiguredCommands['getSnapshot']> => {
		const entries = configured.getSnapshot();
		if (entries !== listed) {
			listed = entries;
			offered = entries
				.filter(({ action }) => action !== 'popup')
				.map(({ bindings, description, display, id }) => ({ bindings, description, display, id }));
		}
		return offered;
	};

	/** The terminal the shown Panel shows, which the Server reads as the command's source. */
	const sourceOf = ({
		selection,
	}: ServerClientViewSample): Omit<
		ServerConfiguredCommandSource,
		'clientId' | 'viewRevision'
	> | null => {
		if (selection === null || selection.tabId === null || selection.panelId === null) return null;
		const panel = {
			kind: 'panel',
			panelId: selection.panelId,
			tabId: selection.tabId,
			terminalContextId: selection.terminalContextId,
		} as const;
		const presentationId = readWorkbenchShownPresentationId(runtime, panel);
		if (presentationId === null) return null;
		// A terminal not started yet, or one that failed to start, gives a command nothing to start from.
		const terminal = findServerTerminalBinding(session.snapshot, presentationId);
		if (terminal === null) return null;
		return {
			panelId: panel.panelId,
			presentationId,
			tabId: panel.tabId,
			terminal: { terminalId: terminal.terminalId, workerId: terminal.workerId },
			terminalContextId: panel.terminalContextId,
		};
	};

	/** As in the TUI, the Pane a command opened takes the focus; a focus that fails says so. */
	const opened = async (outcome: PaneOutcome): Promise<WorkbenchConfiguredCommandOutcome> => {
		if (outcome.kind !== 'panel-split-created') return FAILED;
		if (outcome.terminalOutcome.kind === 'outcome-unknown') return UNKNOWN;
		if (outcome.terminalOutcome.kind !== 'started') return FAILED;
		if (BigInt(session.snapshot.revision) < BigInt(outcome.revision)) {
			await Promise.race([
				session.published(outcome.revision),
				wait(PUBLICATION_WAIT_MILLISECONDS),
			]);
		}
		if (disposed) return FAILED;
		const shown = showWorkbenchClientTarget(runtime, {
			kind: 'panel',
			panelId: outcome.newPanelId,
			tabId: outcome.tabId,
			terminalContextId: outcome.terminalContextId,
		});
		return shown.kind === 'shown' ? ACCEPTED : FAILED;
	};

	const settle = async (
		reply: ServerConfiguredCommandReply,
	): Promise<WorkbenchConfiguredCommandOutcome> => {
		if (reply.kind === 'unsent') return refused('unavailable');
		if (reply.kind === 'unknown') return UNKNOWN;
		const { outcome } = reply;
		switch (outcome.kind) {
			case 'failed':
				// The window offered a command the Server no longer holds, so it reads the list again.
				if (outcome.reason === 'not-found' || outcome.reason === 'conflict') configured.refresh();
				return outcome.reason === 'physical-unknown' ? UNKNOWN : refused(outcome.reason);
			case 'shell-started':
				return ACCEPTED;
			case 'plugin':
				if (outcome.result.outcome.kind === 'failed') return FAILED;
				return outcome.result.outcome.kind === 'unknown' ? UNKNOWN : ACCEPTED;
			case 'pane':
				return opened(outcome.result.outcome);
			default:
				// Only the receipt of the command's own kind confirms that it started.
				return FAILED;
		}
	};

	const invoke = async (commandId: string): Promise<WorkbenchConfiguredCommandOutcome> => {
		const entry = configured.getSnapshot().find(({ id }) => id === commandId);
		if (entry === undefined || entry.action === 'popup') return refused('not-found');
		const sample = clientView?.sample() ?? null;
		if (clientView === null || sample === null) return refused('unavailable');
		// Read with the view, so the source is the terminal the person saw when they chose the command.
		const source = sourceOf(sample);
		const binding = await clientView.bind(sample);
		if (disposed) return refused('unavailable');
		if (binding.kind !== 'bound') {
			return refused(binding.kind === 'stale' ? 'stale-source' : 'unavailable');
		}
		return settle(
			await configured.invoke(
				commandId,
				sample.workbenchRevision,
				source === null
					? null
					: { ...source, clientId: binding.clientId, viewRevision: binding.viewRevision },
			),
		);
	};

	return {
		commands: {
			getSnapshot,
			run: async (commandId) => {
				if (disposed) return refused('unavailable');
				if (running) return refused('busy');
				running = true;
				try {
					return await invoke(commandId);
				} finally {
					running = false;
				}
			},
			subscribe: (listener) => configured.subscribe(listener),
		},
		dispose: () => {
			if (disposed) return;
			disposed = true;
			for (const timer of timers) clearTimeout(timer);
			timers.clear();
			configured.dispose();
		},
	};
}
