import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

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
