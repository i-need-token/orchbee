import type { WorkbenchHeldRestoreOutcome, WorkbenchHeldRestores } from '../../workbench';
import type {
	WorkbenchPanelNode,
	WorkbenchPresentation,
	WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import type { ServerWorkbenchAction, ServerWorkbenchSession } from './ServerWorkbenchSession';

type HeldRestoreSession = Pick<ServerWorkbenchSession, 'submit'>;

/**
 * Starts a held restore's terminal as the TUI does: the Server rechecks the saved directory and the
 * hold, so this Client only names the Presentation it showed. Nothing retries; a person acts again.
 */
export function createServerHeldRestores(session: HeldRestoreSession): WorkbenchHeldRestores {
	return {
		restore: async (presentationId, directory) => {
			let agentResume = false;
			const submission = await session.submit((snapshot) => {
				const held = findHeld(snapshot, presentationId);
				if (held === null) return null;
				agentResume = held.agentResume;
				return held.action(directory);
			});
			return heldRestoreOutcome(submission, directory === 'home' && agentResume);
		},
	};
}

function heldRestoreOutcome(
	submission: Awaited<ReturnType<HeldRestoreSession['submit']>>,
	agentDropped: boolean,
): WorkbenchHeldRestoreOutcome {
	if (submission.kind !== 'settled') return { kind: 'not-started' };
	const { outcome } = submission;
	if (outcome.kind === 'panel-terminal-created') {
		return { agentNotResumed: agentDropped, kind: 'created' };
	}
	if (outcome.kind === 'unavailable' && outcome.reason === 'directory-unavailable') {
		return { kind: 'directory-unavailable' };
	}
	return { kind: 'not-started' };
}

interface HeldPresentation {
	readonly action: (directory: 'home' | 'saved') => ServerWorkbenchAction | null;
	readonly agentResume: boolean;
}

/** The hold of `presentationId` as this snapshot shows it, with the Tab and Panel that hold it. */
function findHeld(snapshot: WorkbenchSnapshot, presentationId: string): HeldPresentation | null {
	for (const context of snapshot.terminalContexts) {
		for (const tab of context.tabs) {
			const found = findInNode(tab.root, presentationId);
			if (found === null) continue;
			const failure = found.presentation.launchFailure;
			if (found.presentation.binding !== null || failure?.reason !== 'directory-unavailable') {
				return null;
			}
			return {
				action: (directory) =>
					readServerWorkbenchAction({
						directory,
						kind: 'panel.restore-terminal',
						panelId: found.panelId,
						presentationId,
						tabId: tab.tabId,
						terminalContextId: context.terminalContextId,
					}),
				agentResume: failure.agentResume === true,
			};
		}
	}
	return null;
}

function findInNode(
	node: WorkbenchPanelNode,
	presentationId: string,
): { readonly panelId: string; readonly presentation: WorkbenchPresentation } | null {
	if (node.kind === 'panel') {
		const presentation = [node.primaryPresentation, ...node.childPresentations].find(
			(member) => member.presentationId === presentationId,
		);
		return presentation === undefined ? null : { panelId: node.panelId, presentation };
	}
	for (const slot of node.children) {
		const found = findInNode(slot.node, presentationId);
		if (found !== null) return found;
	}
	return null;
}
