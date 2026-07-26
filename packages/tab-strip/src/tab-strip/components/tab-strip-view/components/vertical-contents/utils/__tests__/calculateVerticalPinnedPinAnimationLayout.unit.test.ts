import { describe, expect, it } from 'vitest';
import { VerticalTabStripCollapseState } from '../../../../../../types/tab-strip-types';
import type { VerticalPinAnimationEntry } from '../buildVerticalPinAnimationEntries';
import { calculateVerticalPinnedPinAnimationLayout } from '../calculateVerticalPinnedPinAnimationLayout';

function entry(
	id: string,
	phase: VerticalPinAnimationEntry['phase'],
	mainAxisScale: number,
	progress: number,
): VerticalPinAnimationEntry {
	return {
		key: id,
		tab: { id, title: id, pinned: true, group: null },
		order: 0,
		phase,
		progress,
		mainAxisScale,
		opacity: mainAxisScale,
		retainedGroup: null,
	};
}

describe('calculateVerticalPinnedPinAnimationLayout', () => {
	it('interpolates siblings while the destination grows from zero main-axis size', () => {
		const layout = calculateVerticalPinnedPinAnimationLayout({
			sizeBounds: { width: 240, height: null },
			collapseState: VerticalTabStripCollapseState.Expanded,
			children: [
				entry('first', 'stable', 1, 1),
				entry('second', 'stable', 1, 1),
				entry('destination', 'destination', 0.5, 0.5),
			],
		});

		const first = layout.children.find((child) => child.id === 'first');
		const destination = layout.children.find((child) => child.id === 'destination');
		expect(first?.bounds.width).toBeGreaterThan(73);
		expect(first?.bounds.width).toBeLessThan(112);
		expect(destination?.bounds.height).toBe(16);
	});

	it('shrinks a removed pinned source toward zero without dropping it early', () => {
		const layout = calculateVerticalPinnedPinAnimationLayout({
			sizeBounds: { width: 120, height: null },
			collapseState: VerticalTabStripCollapseState.Expanded,
			children: [entry('source', 'source', 0.25, 0.75)],
		});
		expect(layout.children[0]?.bounds.height).toBe(8);
		expect(layout.hostSize.height).toBe(8);
	});
});
