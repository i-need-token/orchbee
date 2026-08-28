import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	closeDesktopWindow,
	focusDesktopWindow,
	inspectDesktopWindowLayoutInvariantViolations,
	moveDesktopWindowTabs,
	normalizeDesktopWindowLayout,
	parseDesktopWindowLayout,
	updateDesktopWindowGeometry,
	type DesktopWindowLayout,
	type DesktopWindowLayoutDocument,
	type DesktopWindowLayoutMoveDestination,
} from '../desktop-window-layout';

const bounds = { height: 720, width: 1_024, x: 20, y: 30 } as const;
const documentFixture = {
	terminalContexts: [{ tabIds: ['tab-1', 'tab-2', 'tab-3'], terminalContextId: 'context-1' }],
} satisfies DesktopWindowLayoutDocument;

describe('desktop window layout', () => {
	it('strictly parses a bounded canonical durable record', () => {
		const layout = twoWindowLayout();

		expect(parseDesktopWindowLayout(layout)).toEqual(layout);
		expect(parseDesktopWindowLayout({ ...layout, unknown: true })).toBeNull();
		expect(
			parseDesktopWindowLayout({
				...layout,
				windows: [{ ...layout.windows[0], bounds: { ...bounds, width: 100 } }],
			}),
		).toBeNull();
	});

	it('collapses missing or duplicate assignment state into one safe primary window', () => {
		const missing = normalizeDesktopWindowLayout(null, documentFixture, bounds);

		expect(missing).toMatchObject({ changed: true, kind: 'recovered' });
		expect(missing.layout.windows).toHaveLength(1);
		expect(missing.layout.windows[0]?.assignments[0]?.tabIds).toEqual(['tab-1', 'tab-2', 'tab-3']);

		const duplicate = twoWindowLayout({
			firstTabs: ['tab-1', 'tab-2'],
			secondTabs: ['tab-2', 'tab-3'],
		});
		const recovered = normalizeDesktopWindowLayout(duplicate, documentFixture, bounds);
		expect(recovered.kind).toBe('recovered');
		expect(recovered.layout.windows).toHaveLength(1);
		expect(
			inspectDesktopWindowLayoutInvariantViolations(recovered.layout, documentFixture),
		).toEqual([]);
	});

	it('drops stale assignments and routes unassigned tabs to the most recent window', () => {
		const layout = twoWindowLayout({
			firstTabs: ['tab-1', 'removed-tab'],
			secondTabs: ['tab-2'],
		});

		const normalized = normalizeDesktopWindowLayout(layout, documentFixture, bounds);

		expect(normalized).toMatchObject({ changed: true, kind: 'reconciled' });
		expect(normalized.layout.windows[0]?.assignments[0]?.tabIds).toEqual(['tab-1']);
		expect(normalized.layout.windows[1]?.assignments[0]?.tabIds).toEqual(['tab-2', 'tab-3']);
		expect(
			inspectDesktopWindowLayoutInvariantViolations(normalized.layout, documentFixture),
		).toEqual([]);
	});

	it('moves a fragmented batch atomically and closes only emptied source windows', () => {
		const result = moveDesktopWindowTabs(
			twoWindowLayout({ firstTabs: ['tab-1', 'tab-2'], secondTabs: ['tab-3'] }),
			documentFixture,
			['tab-2', 'tab-3'],
			{ bounds: { ...bounds, x: 200 }, kind: 'new' },
		);

		expect(result.kind).toBe('completed');
		if (result.kind !== 'completed') return;
		expect(result.closedWindowIds).toEqual(['desktop-window-2']);
		expect(result.destinationWindowId).toBe('desktop-window-3');
		expect(result.layout.windows.map((window) => window.windowId)).toEqual([
			'desktop-window-1',
			'desktop-window-3',
		]);
		expect(result.layout.windows[1]?.assignments[0]?.tabIds).toEqual(['tab-2', 'tab-3']);
		expect(inspectDesktopWindowLayoutInvariantViolations(result.layout, documentFixture)).toEqual(
			[],
		);
	});

	it('rehomes every tab before removing a non-last window', () => {
		const result = closeDesktopWindow(twoWindowLayout(), documentFixture, 'desktop-window-2');

		expect(result.kind).toBe('completed');
		if (result.kind !== 'completed') return;
		expect(result.destinationWindowId).toBe('desktop-window-1');
		expect(result.layout.windows).toHaveLength(1);
		expect(result.layout.windows[0]?.assignments[0]?.tabIds).toEqual(['tab-1', 'tab-2', 'tab-3']);
		expect(closeDesktopWindow(result.layout, documentFixture, 'desktop-window-1')).toEqual({
			kind: 'last-window',
		});
	});

	it('focuses and captures geometry without accepting stale targets', () => {
		const layout = twoWindowLayout();
		const focused = focusDesktopWindow(layout, 'desktop-window-1');
		expect(focused.kind).toBe('completed');
		if (focused.kind !== 'completed') return;
		expect(focused.layout.activationOrder).toEqual(['desktop-window-2', 'desktop-window-1']);

		const geometry = updateDesktopWindowGeometry(
			focused.layout,
			'desktop-window-1',
			{ ...bounds, height: 800 },
			true,
		);
		expect(geometry.kind).toBe('completed');
		expect(focusDesktopWindow(layout, 'stale-window')).toEqual({
			kind: 'blocked',
			reason: 'stale-window',
		});
	});

	it('preserves an exclusive complete assignment across generated traces', () => {
		const tabUniverse = Array.from({ length: 8 }, (_, index) => `tab-${index}`);
		const commandArbitrary = fc.oneof(
			fc.record({ kind: fc.constant('add' as const), tabIndex: fc.nat(20) }),
			fc.record({ kind: fc.constant('remove' as const), tabIndex: fc.nat(20) }),
			fc.record({ kind: fc.constant('focus' as const), windowIndex: fc.nat(20) }),
			fc.record({ kind: fc.constant('close' as const), windowIndex: fc.nat(20) }),
			fc.record({
				destination: fc.constantFrom<'existing' | 'new'>('existing', 'new'),
				kind: fc.constant('move' as const),
				tabIndexes: fc.uniqueArray(fc.nat(20), { maxLength: 4 }),
				windowIndex: fc.nat(20),
			}),
			fc.record({
				height: fc.integer({ max: 1_200, min: 240 }),
				kind: fc.constant('geometry' as const),
				maximized: fc.boolean(),
				width: fc.integer({ max: 1_600, min: 320 }),
				windowIndex: fc.nat(20),
				x: fc.integer({ max: 2_000, min: -2_000 }),
				y: fc.integer({ max: 2_000, min: -2_000 }),
			}),
		);

		fc.assert(
			fc.property(fc.array(commandArbitrary, { maxLength: 60 }), (commands) => {
				let document: DesktopWindowLayoutDocument = {
					terminalContexts: [{ tabIds: tabUniverse.slice(0, 3), terminalContextId: 'context-1' }],
				};
				let layout = normalizeDesktopWindowLayout(null, document, bounds).layout;
				for (const [index, command] of commands.entries()) {
					const currentTabs = document.terminalContexts[0]?.tabIds ?? [];
					if (command.kind === 'add') {
						const tabId = tabUniverse[command.tabIndex % tabUniverse.length];
						if (tabId !== undefined && !currentTabs.includes(tabId)) {
							document = documentWithTabs(
								tabUniverse.filter((candidate) => [...currentTabs, tabId].includes(candidate)),
							);
							layout = normalizeDesktopWindowLayout(layout, document, bounds).layout;
						}
					} else if (command.kind === 'remove') {
						const tabId = tabUniverse[command.tabIndex % tabUniverse.length];
						if (tabId !== undefined && currentTabs.includes(tabId)) {
							document = documentWithTabs(currentTabs.filter((candidate) => candidate !== tabId));
							layout = normalizeDesktopWindowLayout(layout, document, bounds).layout;
						}
					} else if (command.kind === 'focus') {
						const window = layout.windows[command.windowIndex % layout.windows.length];
						if (window !== undefined) {
							const result = focusDesktopWindow(layout, window.windowId);
							if (result.kind === 'completed') layout = result.layout;
						}
					} else if (command.kind === 'close') {
						const window = layout.windows[command.windowIndex % layout.windows.length];
						if (window !== undefined) {
							const result = closeDesktopWindow(layout, document, window.windowId);
							if (result.kind === 'completed') layout = result.layout;
						}
					} else if (command.kind === 'move') {
						const selectedTabIds = command.tabIndexes.flatMap((tabIndex) => {
							const tabId = tabUniverse[tabIndex % tabUniverse.length];
							return tabId === undefined ? [] : [tabId];
						});
						const existingWindow = layout.windows[command.windowIndex % layout.windows.length];
						let destination: DesktopWindowLayoutMoveDestination | null = null;
						if (command.destination === 'new') {
							destination = { bounds: { ...bounds, x: command.windowIndex }, kind: 'new' };
						} else if (existingWindow !== undefined) {
							destination = { kind: 'existing', windowId: existingWindow.windowId };
						}
						if (destination !== null) {
							const result = moveDesktopWindowTabs(layout, document, selectedTabIds, destination);
							if (result.kind === 'completed') layout = result.layout;
						}
					} else {
						const window = layout.windows[command.windowIndex % layout.windows.length];
						if (window !== undefined) {
							const result = updateDesktopWindowGeometry(
								layout,
								window.windowId,
								{
									height: command.height,
									width: command.width,
									x: command.x,
									y: command.y,
								},
								command.maximized,
							);
							if (result.kind === 'completed') layout = result.layout;
						}
					}
					const details = JSON.stringify({ command, document, index, layout });
					const violations = inspectDesktopWindowLayoutInvariantViolations(layout, document);
					if (violations.length > 0) throw new Error(details);
					expect(violations).toEqual([]);
					const parsed = parseDesktopWindowLayout(JSON.parse(JSON.stringify(layout)));
					if (parsed === null) throw new Error(details);
					expect(parsed).not.toBeNull();
					expect(normalizeDesktopWindowLayout(parsed, document, bounds).changed).toBe(false);
				}
			}),
			{ numRuns: 250, seed: 0x5a17_2026 },
		);
	});
});

function documentWithTabs(tabIds: readonly string[]): DesktopWindowLayoutDocument {
	return { terminalContexts: [{ tabIds, terminalContextId: 'context-1' }] };
}

function twoWindowLayout(
	options: {
		readonly firstTabs?: readonly string[];
		readonly secondTabs?: readonly string[];
	} = {},
): DesktopWindowLayout {
	return {
		activationOrder: ['desktop-window-1', 'desktop-window-2'],
		nextWindowSequence: 3,
		primaryWindowId: 'desktop-window-1',
		version: 1,
		windows: [
			{
				assignments: [
					{
						tabIds: options.firstTabs ?? ['tab-1', 'tab-2'],
						terminalContextId: 'context-1',
					},
				],
				bounds,
				maximized: false,
				ordinal: 1,
				windowId: 'desktop-window-1',
			},
			{
				assignments: [
					{
						tabIds: options.secondTabs ?? ['tab-3'],
						terminalContextId: 'context-1',
					},
				],
				bounds: { ...bounds, x: 1_100 },
				maximized: true,
				ordinal: 2,
				windowId: 'desktop-window-2',
			},
		],
	};
}
