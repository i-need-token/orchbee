import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	MessageKind,
	TerminalIdSchema,
	TerminalPhase,
	type TerminalInventoryCursor,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { HostConnection } from '../HostConnection';
import { readTerminalInventory } from '../readTerminalInventory';

const hostEpoch = HostEpochSchema.parse('11'.repeat(16));

describe('readTerminalInventory', () => {
	it('reads every page from one generation without imposing a total count ceiling', async () => {
		const first = createSummary(1);
		const second = createSummary(2);
		const cursors: Array<TerminalInventoryCursor | null> = [];
		const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(
			async (kind, value) => {
				expect(kind).toBe(MessageKind.listTerminals);
				const cursor = (value as { readonly cursor: TerminalInventoryCursor | null }).cursor;
				cursors.push(cursor);
				return cursor === null
					? {
							inventoryGeneration: '7',
							nextCursor: { inventoryGeneration: '7', offset: '1' },
							terminals: [first],
						}
					: { inventoryGeneration: '7', nextCursor: null, terminals: [second] };
			},
		);

		expect(await readTerminalInventory(createHost(requestControl))).toEqual([first, second]);
		expect(cursors).toEqual([null, { inventoryGeneration: '7', offset: '1' }]);
	});

	it('rejects cross-generation and repeated-cursor scans instead of returning partial inventory', async () => {
		let requestCount = 0;
		const crossGeneration = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(async () => {
			requestCount += 1;
			return requestCount === 1
				? {
						inventoryGeneration: '7',
						nextCursor: { inventoryGeneration: '7', offset: '1' },
						terminals: [],
					}
				: { inventoryGeneration: '8', nextCursor: null, terminals: [] };
		});
		await expect(readTerminalInventory(createHost(crossGeneration))).rejects.toThrow(
			'changed generation',
		);

		requestCount = 0;
		const repeatedCursor = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(async () => {
			requestCount += 1;
			return {
				inventoryGeneration: '7',
				nextCursor: { inventoryGeneration: '7', offset: '1' },
				terminals: [],
			};
		});
		await expect(readTerminalInventory(createHost(repeatedCursor))).rejects.toThrow(
			'repeated a pagination cursor',
		);
	});
});

function createHost(
	requestControl: (...arguments_: unknown[]) => Promise<unknown>,
): HostConnection {
	return { hostEpoch, requestControl } as unknown as HostConnection;
}

function createSummary(index: number): TerminalSummary {
	return {
		columns: 80,
		exit: null,
		identity: {
			creationId: CreationIdSchema.parse(index.toString(16).padStart(32, '0')),
			hostEpoch,
			terminalId: TerminalIdSchema.parse((index + 16).toString(16).padStart(32, '0')),
		},
		phase: TerminalPhase.running,
		revision: CanonicalUint64TextSchema.parse('1'),
		rows: 24,
		terminateRequested: false,
	};
}
