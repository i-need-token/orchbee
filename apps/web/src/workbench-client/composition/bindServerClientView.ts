import {
	readWorkbenchClientSelection,
	showWorkbenchClientTarget,
	type WorkbenchClientTarget,
	type WorkbenchProjectWorkspaceRuntime,
} from '@orchbee/workbench';

import type { ServerConnection } from '../server-connection';
import {
	ServerClientView,
	type ServerClientViewBinding,
	type ServerClientViewControl,
	type ServerClientViewOutcome,
	type ServerClientViewSample,
	type ServerWorkbenchSession,
} from '../server-workbench';

/** How long a control waits for the publication it names before the window gives up on it. */
const PUBLICATION_WAIT_MILLISECONDS = 5_000;

type Reason = Extract<ServerClientViewOutcome, { readonly kind: 'rejected' }>['reason'];

interface Answer {
	readonly outcome: ServerClientViewOutcome;
	/** The view the outcome was observed in; null sends the window's view when the answer leaves. */
	readonly view: ServerClientViewSample | null;
}

/** The view this window publishes, which a configured command names as its source. */
export interface BoundServerClientView {
	/** Binds a view the window showed to the Server's record of it, as the TUI binds a command. */
	bind(sample: ServerClientViewSample): Promise<ServerClientViewBinding>;
	dispose(): void;
	/** What the window shows now, or null while that is not settled enough to report. */
	sample(): ServerClientViewSample | null;
}

/**
 * Reports what this window shows to the Server and shows what the CLI asks of it, so that a Workspace,
 * Tab or Panel focus and a zoom reach the window as they reach the TUI. The window reports itself ready
 * throughout and answers a control once the change is drawn: after React commits what the runtime now
 * holds. Returns null when the connection holds no lane for the view.
 */
export function bindServerClientView({
	connection,
	runtime,
	session,
}: {
	readonly connection: Pick<ServerConnection, 'closeLane' | 'exchange' | 'openLane' | 'subscribe'>;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
	readonly session: Pick<ServerWorkbenchSession, 'published' | 'snapshot' | 'subscribe'>;
}): BoundServerClientView | null {
	let disposed = false;
	const timers = new Set<ReturnType<typeof setTimeout>>();
	const wait = (milliseconds: number): Promise<void> =>
		new Promise((resolve) => {
			const timer = setTimeout(() => {
				timers.delete(timer);
				resolve();
			}, milliseconds);
			timers.add(timer);
		});
	const sample = (): ServerClientViewSample | null => {
		const selection = readWorkbenchClientSelection(runtime);
		return selection === undefined
			? null
			: { phase: 'ready', selection, workbenchRevision: session.snapshot.revision };
	};
	const execute = async ({
		intent,
		minimumWorkbenchRevision,
	}: ServerClientViewControl): Promise<Answer> => {
		// As in the TUI, an answer claims a partial change once the window moved for the control.
		let effect = false;
		const failed = (reason: Reason): Answer => ({
			outcome: { kind: effect ? 'incomplete' : 'rejected', reason },
			view: null,
		});
		/** Shows the target and waits until React has drawn it; false when the target is missing. */
		const show = async (next: WorkbenchClientTarget): Promise<boolean> => {
			const navigation = showWorkbenchClientTarget(runtime, next);
			if (navigation.kind === 'unavailable') return false;
			if (navigation.changed) {
				effect = true;
				// React commits a store change within the task that made it, so the next task sees it drawn.
				await wait(0);
			}
			return true;
		};
		let target = navigationTarget(intent);
		if (target === null) return failed('unsupported');
		const minimum = BigInt(minimumWorkbenchRevision);
		if (BigInt(session.snapshot.revision) < minimum) {
			await Promise.race([
				session.published(minimumWorkbenchRevision),
				wait(PUBLICATION_WAIT_MILLISECONDS),
			]);
			if (disposed) return failed('interrupted');
			if (BigInt(session.snapshot.revision) < minimum) return failed('projection-unavailable');
		}
		if (!(await show(target))) return failed('target-unavailable');
		let outcome: ServerClientViewOutcome = { kind: 'applied' };
		if (target.kind === 'panel' && (intent.kind === 'toggle-zoom' || intent.kind === 'zoom')) {
			const selection = readWorkbenchClientSelection(runtime);
			const previouslyZoomed = selection?.zoomed ?? false;
			if (selection?.panelCount === 1 && (intent.kind === 'toggle-zoom' || intent.enabled)) {
				// As in the TUI, a Tab of one Panel has nothing to zoom.
				outcome = { kind: 'zoom-unchanged' };
			} else if (intent.kind === 'toggle-zoom') {
				// The zoom followed the focus onto the Panel, so the toggle starts from the Tab's own zoom.
				target = { ...target, zoomed: !previouslyZoomed };
				if (!(await show(target))) return failed('target-unavailable');
				outcome = { kind: 'zoom-toggled', previouslyZoomed };
			}
		}
		const view = sample();
		if (view !== null && observed(target, minimum, view, outcome)) return { outcome, view };
		// Something else moved the window before the change was drawn.
		return failed('interrupted');
	};

	const clientView = ServerClientView.open({
		connection,
		onControl: (control) => {
			void execute(control).then(
				({ outcome, view }) => clientView?.complete(control.operationId, outcome, view),
				() =>
					clientView?.complete(control.operationId, { kind: 'incomplete', reason: 'interrupted' }),
			);
		},
		sample,
		sessionId: session.snapshot.sessionId,
	});
	if (clientView === null) return null;
	const changed = (): void => clientView.changed();
	const stopView = runtime.subscribeClientView(changed);
	const stopPublication = session.subscribe(changed);
	return {
		bind: (shown) => clientView.bind(shown),
		dispose: () => {
			if (disposed) return;
			disposed = true;
			stopView();
			stopPublication();
			for (const timer of timers) clearTimeout(timer);
			timers.clear();
			clientView.dispose();
		},
		sample,
	};
}

/** Where a navigation intent asks the window to look; null for what this window does not run yet. */
function navigationTarget(intent: ServerClientViewControl['intent']): WorkbenchClientTarget | null {
	switch (intent.kind) {
		case 'context':
			return { kind: 'context', terminalContextId: intent.terminalContextId };
		case 'tab':
			return { kind: 'tab', tabId: intent.tabId, terminalContextId: intent.terminalContextId };
		case 'panel':
		case 'toggle-zoom':
		case 'zoom':
			return {
				kind: 'panel',
				panelId: intent.panelId,
				tabId: intent.tabId,
				terminalContextId: intent.terminalContextId,
				zoomed: intent.kind === 'zoom' ? intent.enabled : undefined,
			};
		default:
			return null;
	}
}

/** Whether the drawn view shows the target as the outcome claims, which the Server checks too. */
function observed(
	target: WorkbenchClientTarget,
	minimum: bigint,
	{ selection, workbenchRevision }: ServerClientViewSample,
	outcome: ServerClientViewOutcome,
): boolean {
	if (
		selection === null ||
		BigInt(workbenchRevision) < minimum ||
		selection.terminalContextId !== target.terminalContextId ||
		(target.kind !== 'context' && selection.tabId !== target.tabId) ||
		(target.kind === 'panel' && selection.panelId !== target.panelId)
	) {
		return false;
	}
	switch (outcome.kind) {
		case 'zoom-toggled':
			return selection.panelCount > 1 && selection.zoomed !== outcome.previouslyZoomed;
		case 'zoom-unchanged':
			return selection.panelCount === 1 && !selection.zoomed;
		default:
			return (
				target.kind !== 'panel' || target.zoomed === undefined || selection.zoomed === target.zoomed
			);
	}
}
