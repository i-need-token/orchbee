import { TabActive, TabPinned } from '../model';
import { DEFAULT_TAB_STYLE_METRICS, type TabStyleMetrics } from '../style';
import type { TabLayoutState } from './TabLayoutState';
import type { TabSizeInfo } from './tab-strip-layout-types';

/** Port of TabWidthConstraints in tab_width_constraints.{h,cc}. */
export class TabWidthConstraints {
	public readonly state: TabLayoutState;
	private readonly sizeInfo: Readonly<TabSizeInfo>;
	private readonly style: Readonly<TabStyleMetrics>;

	public constructor(
		state: TabLayoutState,
		sizeInfo: Readonly<TabSizeInfo>,
		style: Readonly<TabStyleMetrics> = DEFAULT_TAB_STYLE_METRICS,
	) {
		this.state = state;
		this.sizeInfo = sizeInfo;
		this.style = style;
	}

	/** The smallest width this tab should ever have. */
	public getMinimumWidth(): number {
		const minimum =
			this.state.active === TabActive.Active
				? this.sizeInfo.minActiveWidth
				: this.sizeInfo.minInactiveWidth;
		return this.transformForPinnednessAndOpenness(minimum);
	}

	/** Width at the crossover between the two tab-strip layout domains. */
	public getLayoutCrossoverWidth(): number {
		return this.transformForPinnednessAndOpenness(this.sizeInfo.minActiveWidth);
	}

	/** The width this tab would use when all preferred space is available. */
	public getPreferredWidth(): number {
		return this.transformForPinnednessAndOpenness(this.sizeInfo.standardWidth);
	}

	private transformForPinnednessAndOpenness(width: number): number {
		if (this.state.isClosed()) {
			return this.style.tabOverlap;
		}
		if (this.state.pinned === TabPinned.Pinned) {
			return this.sizeInfo.pinnedTabWidth;
		}
		return width;
	}
}
