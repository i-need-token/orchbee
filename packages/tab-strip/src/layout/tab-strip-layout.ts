import type { Rect } from '../geometry';
import { rectRight } from '../geometry';
import { DEFAULT_TAB_STYLE_METRICS, type TabStyleMetrics } from '../style';
import { LayoutDomain } from './tab-strip-layout-types';
import type { TabWidthConstraints } from './TabWidthConstraints';

function clamp(value: number, low: number, high: number): number {
	return Math.min(Math.max(value, low), high);
}

function floatValueBetween(fraction: number, start: number, end: number): number {
	return start + (end - start) * fraction;
}

/** Port of Chromium's TabSizer. */
export class TabSizer {
	public readonly domain: LayoutDomain;
	private readonly spaceFractionAvailable: number;

	public constructor(domain: LayoutDomain, spaceFractionAvailable: number) {
		this.domain = domain;
		this.spaceFractionAvailable = spaceFractionAvailable;
	}

	public calculateTabWidth(tab: TabWidthConstraints): number {
		switch (this.domain) {
			case LayoutDomain.InactiveWidthBelowActiveWidth:
				return Math.floor(
					floatValueBetween(
						this.spaceFractionAvailable,
						tab.getMinimumWidth(),
						tab.getLayoutCrossoverWidth(),
					),
				);
			case LayoutDomain.InactiveWidthEqualsActiveWidth:
				return Math.floor(
					floatValueBetween(
						this.spaceFractionAvailable,
						tab.getLayoutCrossoverWidth(),
						tab.getPreferredWidth(),
					),
				);
		}
	}

	public tabAcceptsExtraSpace(tab: TabWidthConstraints): boolean {
		if (this.spaceFractionAvailable === 0 || this.spaceFractionAvailable === 1) {
			return false;
		}

		switch (this.domain) {
			case LayoutDomain.InactiveWidthBelowActiveWidth:
				return tab.getMinimumWidth() < tab.getLayoutCrossoverWidth();
			case LayoutDomain.InactiveWidthEqualsActiveWidth:
				return tab.getLayoutCrossoverWidth() < tab.getPreferredWidth();
		}
	}

	public isAlreadyPreferredWidth(): boolean {
		return (
			this.domain === LayoutDomain.InactiveWidthEqualsActiveWidth &&
			this.spaceFractionAvailable === 1
		);
	}
}

export interface TabBoundsResult {
	bounds: Rect[];
	domain: LayoutDomain;
}

function calculateSpaceFractionAvailable(
	tabs: readonly TabWidthConstraints[],
	width: number | null,
	style: Readonly<TabStyleMetrics>,
): TabSizer {
	if (width === null) {
		return new TabSizer(LayoutDomain.InactiveWidthEqualsActiveWidth, 1);
	}

	let minimumWidth = 0;
	let crossoverWidth = 0;
	let preferredWidth = 0;
	for (const tab of tabs) {
		// Subtract the trailing foot; the next tab overlaps it.
		minimumWidth += tab.getMinimumWidth() - style.tabOverlap;
		crossoverWidth += tab.getLayoutCrossoverWidth() - style.tabOverlap;
		preferredWidth += tab.getPreferredWidth() - style.tabOverlap;
	}

	// Add back the final tab's trailing foot.
	minimumWidth += style.tabOverlap;
	crossoverWidth += style.tabOverlap;
	preferredWidth += style.tabOverlap;

	let domain: LayoutDomain;
	let fraction: number;
	if (width < crossoverWidth) {
		domain = LayoutDomain.InactiveWidthBelowActiveWidth;
		fraction =
			minimumWidth === crossoverWidth
				? 1
				: (width - minimumWidth) / (crossoverWidth - minimumWidth);
	} else {
		domain = LayoutDomain.InactiveWidthEqualsActiveWidth;
		fraction =
			preferredWidth === crossoverWidth
				? 1
				: (width - crossoverWidth) / (preferredWidth - crossoverWidth);
	}

	return new TabSizer(domain, clamp(fraction, 0, 1));
}

/**
 * Retroactively distributes pixels lost by flooring, from left to right.
 * This is intentionally index ordered to match AllocateExtraSpace in C++.
 */
function allocateExtraSpace(
	bounds: Rect[],
	tabs: readonly TabWidthConstraints[],
	extraSpace: number | null,
	tabSizer: TabSizer,
): void {
	if (tabSizer.isAlreadyPreferredWidth() || extraSpace === null || extraSpace <= 0) {
		return;
	}

	let allocatedExtraSpace = 0;
	for (let index = 0; index < tabs.length; index += 1) {
		const boundsAtIndex = bounds[index];
		const tab = tabs[index];
		if (!boundsAtIndex || !tab) {
			throw new Error('Tab bounds and constraints must have matching lengths.');
		}

		boundsAtIndex.x += allocatedExtraSpace;
		if (allocatedExtraSpace < extraSpace && tabSizer.tabAcceptsExtraSpace(tab)) {
			allocatedExtraSpace += 1;
			boundsAtIndex.width += 1;
		}
	}
}

/**
 * Calculates horizontal tab bounds exactly as CalculateTabBounds in Chromium.
 * A null width means unconstrained preferred-width layout.
 */
export function calculateTabBounds(
	tabs: readonly TabWidthConstraints[],
	width: number | null,
	style: Readonly<TabStyleMetrics> = DEFAULT_TAB_STYLE_METRICS,
): TabBoundsResult {
	if (tabs.length === 0) {
		return {
			bounds: [],
			domain: LayoutDomain.InactiveWidthEqualsActiveWidth,
		};
	}

	const tabSizer = calculateSpaceFractionAvailable(tabs, width, style);
	let nextX = 0;
	const bounds: Rect[] = tabs.map((tab) => {
		const tabWidth = tabSizer.calculateTabWidth(tab);
		const result = { x: nextX, y: 0, width: tabWidth, height: style.standardHeight };
		nextX += tabWidth - style.tabOverlap;
		return result;
	});

	const lastBounds = bounds.at(-1);
	if (!lastBounds) {
		throw new Error('Non-empty constraints must produce non-empty bounds.');
	}
	const extraSpace = width === null ? null : width - rectRight(lastBounds);
	allocateExtraSpace(bounds, tabs, extraSpace, tabSizer);

	return { bounds, domain: tabSizer.domain };
}
