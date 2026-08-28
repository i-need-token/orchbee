import { describe, expect, it } from 'vitest';

import {
	DesktopWindowLayoutMainEventSchema,
	DesktopWindowLayoutRendererCommandSchema,
	DesktopWindowLayoutSnapshotSchema,
} from '../desktop-window-layout-contract';

describe('Desktop window-layout contract', () => {
	it('parses window projections and correlated Tab movement', () => {
		const snapshot = DesktopWindowLayoutSnapshotSchema.parse({
			currentWindowId: 'desktop-window-2',
			revision: '8',
			terminalContexts: [{ tabIds: ['context-1-tab-2'], terminalContextId: 'context-1' }],
			windows: [
				{ ordinal: 1, windowId: 'desktop-window-1' },
				{ ordinal: 2, windowId: 'desktop-window-2' },
			],
		});
		const command = DesktopWindowLayoutRendererCommandSchema.parse({
			destination: { kind: 'existing', windowId: 'desktop-window-1' },
			requestId: 'window-layout:move:1',
			tabIds: snapshot.terminalContexts[0]?.tabIds,
			terminalContextId: 'context-1',
			type: 'desktop-window-layout-move-tabs-requested',
		});
		if (command.type !== 'desktop-window-layout-move-tabs-requested') {
			throw new Error('Expected a Tab-move command.');
		}

		expect(command).toMatchObject({
			destination: { windowId: 'desktop-window-1' },
			tabIds: ['context-1-tab-2'],
		});
		expect(
			DesktopWindowLayoutMainEventSchema.parse({
				requestId: command.requestId,
				result: { destinationWindowId: 'desktop-window-1', kind: 'completed' },
				type: 'desktop-window-layout-operation-responded',
			}),
		).toMatchObject({ result: { kind: 'completed' } });
	});

	it('parses bounded drag lifecycle messages', () => {
		expect(
			DesktopWindowLayoutRendererCommandSchema.parse({
				point: { x: 144.5, y: -20 },
				sessionId: 'window-layout:drag:1',
				type: 'desktop-window-layout-drag-updated',
			}),
		).toMatchObject({ point: { x: 144.5 } });
		expect(
			DesktopWindowLayoutMainEventSchema.parse({
				sessionId: 'window-layout:drag:1',
				target: { kind: 'new' },
				type: 'desktop-window-layout-drag-target-changed',
			}),
		).toMatchObject({ target: { kind: 'new' } });
	});

	it('rejects ambiguous projections and malformed operations', () => {
		expect(
			DesktopWindowLayoutSnapshotSchema.safeParse({
				currentWindowId: 'missing-window',
				revision: '1',
				terminalContexts: [
					{ tabIds: ['duplicate-tab'], terminalContextId: 'context-1' },
					{ tabIds: ['duplicate-tab'], terminalContextId: 'context-2' },
				],
				windows: [{ ordinal: 1, windowId: 'desktop-window-1' }],
			}).success,
		).toBe(false);
		expect(
			DesktopWindowLayoutRendererCommandSchema.safeParse({
				destination: { kind: 'new' },
				requestId: 'window-layout:move:2',
				tabIds: ['tab-1', 'tab-1'],
				terminalContextId: 'context-1',
				type: 'desktop-window-layout-move-tabs-requested',
			}).success,
		).toBe(false);
		expect(
			DesktopWindowLayoutRendererCommandSchema.safeParse({
				sessionId: 'window-layout:drag:2',
				type: 'desktop-window-layout-drag-completed',
			}).success,
		).toBe(false);
	});
});
