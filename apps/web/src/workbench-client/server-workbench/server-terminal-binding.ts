import type { WorkbenchPanelNode, WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import type { ServerTerminalBinding } from '../server-connection';
import { findServerPresentation } from './findServerPresentation';

/** The terminal a Presentation is bound to in this snapshot, if it holds one. */
export function findServerTerminalBinding(
	snapshot: WorkbenchSnapshot,
	presentationId: string,
): ServerTerminalBinding | null {
	const binding = findServerPresentation(snapshot, presentationId)?.binding ?? null;
	return binding === null ? null : { sessionId: snapshot.sessionId, ...binding };
}

/** Names one terminal of a session, so a Presentation bound to another one names another. */
export function serverTerminalBindingKey(binding: {
	readonly terminalId: string;
	readonly workerId: string;
}): string {
	return `${binding.workerId}:${binding.terminalId}`;
}

/**
 * Every Presentation in this snapshot, in every Workspace, with the key of the terminal it is bound
 * to, or null where it holds none.
 */
export function listServerTerminalBindings(
	snapshot: WorkbenchSnapshot,
): Map<string, string | null> {
	const terminals = new Map<string, string | null>();
	const collect = (node: WorkbenchPanelNode): void => {
		if (node.kind === 'panel') {
			for (const { binding, presentationId } of [
				node.primaryPresentation,
				...node.childPresentations,
			]) {
				terminals.set(presentationId, binding === null ? null : serverTerminalBindingKey(binding));
			}
			return;
		}
		for (const slot of node.children) collect(slot.node);
	};
	for (const context of snapshot.terminalContexts) {
		for (const tab of context.tabs) collect(tab.root);
	}
	return terminals;
}
