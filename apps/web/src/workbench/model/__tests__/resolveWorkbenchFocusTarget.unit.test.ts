import { describe, expect, it } from 'vitest';

import {
	resolveWorkbenchFocusTarget,
	type WorkbenchFocusRegion,
} from '../resolveWorkbenchFocusTarget';

const WORKBENCH_FOCUS_REGIONS = [
	'terminal',
	'tab-strip',
	'sidebar',
] as const satisfies readonly WorkbenchFocusRegion[];

describe('resolveWorkbenchFocusTarget', () => {
	it('cycles through the canonical available region order in both directions', () => {
		for (const [currentRegion, next, previous] of [
			['terminal', 'tab-strip', 'sidebar'],
			['tab-strip', 'sidebar', 'terminal'],
			['sidebar', 'terminal', 'tab-strip'],
		] as const) {
			expect(
				resolveWorkbenchFocusTarget({
					availableRegions: WORKBENCH_FOCUS_REGIONS,
					currentRegion,
					exclusiveSurfaceActive: false,
					intent: { direction: 'next', kind: 'cycle' },
				}),
			).toEqual({ kind: 'target', region: next });
			expect(
				resolveWorkbenchFocusTarget({
					availableRegions: WORKBENCH_FOCUS_REGIONS,
					currentRegion,
					exclusiveSurfaceActive: false,
					intent: { direction: 'previous', kind: 'cycle' },
				}),
			).toEqual({ kind: 'target', region: previous });
		}
	});

	it('skips every unavailable region without trusting caller order or duplicates', () => {
		expect(
			resolveWorkbenchFocusTarget({
				availableRegions: ['sidebar', 'terminal', 'terminal'],
				currentRegion: 'terminal',
				exclusiveSurfaceActive: false,
				intent: { direction: 'next', kind: 'cycle' },
			}),
		).toEqual({ kind: 'target', region: 'sidebar' });
		expect(
			resolveWorkbenchFocusTarget({
				availableRegions: ['sidebar', 'terminal'],
				currentRegion: 'terminal',
				exclusiveSurfaceActive: false,
				intent: { direction: 'previous', kind: 'cycle' },
			}),
		).toEqual({ kind: 'target', region: 'sidebar' });
	});

	it('anchors unknown and stale focus at the first canonical available region', () => {
		for (const direction of ['next', 'previous'] as const) {
			expect(
				resolveWorkbenchFocusTarget({
					availableRegions: ['tab-strip', 'sidebar'],
					currentRegion: 'terminal',
					exclusiveSurfaceActive: false,
					intent: { direction, kind: 'cycle' },
				}),
			).toEqual({ kind: 'target', region: 'tab-strip' });
			expect(
				resolveWorkbenchFocusTarget({
					availableRegions: WORKBENCH_FOCUS_REGIONS,
					currentRegion: null,
					exclusiveSurfaceActive: false,
					intent: { direction, kind: 'cycle' },
				}),
			).toEqual({ kind: 'target', region: 'terminal' });
		}
	});

	it('rejects empty and singleton cycles', () => {
		expect(
			resolveWorkbenchFocusTarget({
				availableRegions: [],
				currentRegion: null,
				exclusiveSurfaceActive: false,
				intent: { direction: 'next', kind: 'cycle' },
			}),
		).toEqual({ kind: 'unavailable', reason: 'no-available-region' });
		expect(
			resolveWorkbenchFocusTarget({
				availableRegions: ['terminal'],
				currentRegion: 'terminal',
				exclusiveSurfaceActive: false,
				intent: { direction: 'next', kind: 'cycle' },
			}),
		).toEqual({ kind: 'unavailable', reason: 'no-different-region' });
	});

	it('resolves direct focus, repeat no-op, and unavailable targets', () => {
		const availableRegions: readonly WorkbenchFocusRegion[] = ['terminal', 'tab-strip'];
		expect(
			resolveWorkbenchFocusTarget({
				availableRegions,
				currentRegion: 'terminal',
				exclusiveSurfaceActive: false,
				intent: { kind: 'direct', region: 'tab-strip' },
			}),
		).toEqual({ kind: 'target', region: 'tab-strip' });
		expect(
			resolveWorkbenchFocusTarget({
				availableRegions,
				currentRegion: 'terminal',
				exclusiveSurfaceActive: false,
				intent: { kind: 'direct', region: 'terminal' },
			}),
		).toEqual({ kind: 'unchanged', region: 'terminal' });
		expect(
			resolveWorkbenchFocusTarget({
				availableRegions,
				currentRegion: 'terminal',
				exclusiveSurfaceActive: false,
				intent: { kind: 'direct', region: 'sidebar' },
			}),
		).toEqual({ kind: 'unavailable', reason: 'target-unavailable' });
	});

	it('keeps exclusive surfaces outside every direct and cyclic route', () => {
		for (const intent of [
			{ direction: 'next', kind: 'cycle' },
			{ kind: 'direct', region: 'tab-strip' },
		] as const) {
			expect(
				resolveWorkbenchFocusTarget({
					availableRegions: WORKBENCH_FOCUS_REGIONS,
					currentRegion: 'terminal',
					exclusiveSurfaceActive: true,
					intent,
				}),
			).toEqual({ kind: 'unavailable', reason: 'exclusive-surface-active' });
		}
	});

	it('exhaustively keeps every accepted cycle target available and different', () => {
		for (let mask = 0; mask < 1 << WORKBENCH_FOCUS_REGIONS.length; mask += 1) {
			const availableRegions = WORKBENCH_FOCUS_REGIONS.filter(
				(_, index) => (mask & (1 << index)) !== 0,
			);
			for (const currentRegion of [null, ...WORKBENCH_FOCUS_REGIONS] as const) {
				for (const direction of ['next', 'previous'] as const) {
					const result = resolveWorkbenchFocusTarget({
						availableRegions,
						currentRegion,
						exclusiveSurfaceActive: false,
						intent: { direction, kind: 'cycle' },
					});
					const currentWasAvailable = availableRegions.includes(
						currentRegion as WorkbenchFocusRegion,
					);
					expect(
						result.kind !== 'target' ||
							(availableRegions.includes(result.region) &&
								(!currentWasAvailable || result.region !== currentRegion)),
					).toBe(true);
				}
			}
		}
	});
});
