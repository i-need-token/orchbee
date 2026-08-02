import {
	ListTerminalsV1_3Schema,
	MessageKind,
	PROTOCOL_LIMITS,
	TerminalListV1_3Schema,
	type TerminalInventoryCursor,
	type TerminalListV1_3,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';

import type { HostConnection } from './HostConnection';

export async function readTerminalInventory(
	host: HostConnection,
): Promise<readonly TerminalSummary[]> {
	const terminals: TerminalSummary[] = [];
	const seenCreationIds = new Set<string>();
	const seenCursors = new Set<string>();
	const seenTerminalIds = new Set<string>();
	let cursor: TerminalInventoryCursor | null = null;
	let inventoryGeneration: string | undefined;
	do {
		const page: TerminalListV1_3 = await host.requestControl(
			MessageKind.listTerminals,
			ListTerminalsV1_3Schema.parse({
				cursor,
				maximumTerminals: PROTOCOL_LIMITS.maxInventoryPageTerminals,
			}),
			MessageKind.listTerminals,
			TerminalListV1_3Schema,
		);
		if (inventoryGeneration !== undefined && page.inventoryGeneration !== inventoryGeneration) {
			throw new Error('Terminal inventory changed generation within one scan.');
		}
		inventoryGeneration = page.inventoryGeneration;
		for (const summary of page.terminals) {
			if (summary.identity.hostEpoch !== host.hostEpoch) {
				throw new Error('Terminal inventory contains a foreign HostEpoch.');
			}
			if (
				seenCreationIds.has(summary.identity.creationId) ||
				seenTerminalIds.has(summary.identity.terminalId)
			) {
				throw new Error('Terminal inventory contains a duplicate identity.');
			}
			seenCreationIds.add(summary.identity.creationId);
			seenTerminalIds.add(summary.identity.terminalId);
			terminals.push(summary);
		}
		cursor = page.nextCursor;
		if (cursor !== null) {
			const cursorKey = `${cursor.inventoryGeneration}:${cursor.offset}`;
			if (seenCursors.has(cursorKey)) {
				throw new Error('Terminal inventory repeated a pagination cursor.');
			}
			seenCursors.add(cursorKey);
		}
	} while (cursor !== null);
	return terminals;
}
