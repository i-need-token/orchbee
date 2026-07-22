import { requireDefined, TabStripOrientation } from '../common';
import { LAYOUT_CONSTANTS } from '../style';
import type { ProposedLayout, Size, SizeBounds } from './proposed-layout';
export const SPLIT_VIEW_GAP = 2;
export interface SplitTabViewLayoutInput {
	orientation: TabStripOrientation;
	sizeBounds: SizeBounds;
	children: readonly [Readonly<Size>, Readonly<Size>];
	visible?: readonly [boolean, boolean];
	collapsed?: boolean;
	pinned?: boolean;
}

/** Port of common/split_tab_view.cc Calculate*Layout. */
export function calculateSplitTabViewLayout(input: SplitTabViewLayoutInput): ProposedLayout {
	return input.orientation === TabStripOrientation.Horizontal
		? calculateHorizontal(input)
		: calculateVertical(input);
}
function calculateHorizontal(input: SplitTabViewLayoutInput): ProposedLayout {
	const height = input.sizeBounds.height ?? LAYOUT_CONSTANTS.tabHeight;
	let x = 0;
	const children = input.children.map((preferred, index) => {
		let width = preferred.width;
		if (input.sizeBounds.width !== null) {
			width = index === 0 ? Math.floor(input.sizeBounds.width / 2) : input.sizeBounds.width - x;
		}
		const bounds = {
			x,
			y: 0,
			width,
			height,
		};
		x += width;
		return {
			id: `split-${index}`,
			visible: input.visible?.[index] ?? true,
			bounds,
		};
	});
	return {
		hostSize: {
			width: x,
			height,
		},
		children,
	};
}
function calculateVertical(input: SplitTabViewLayoutInput): ProposedLayout {
	const border = input.pinned ? LAYOUT_CONSTANTS.verticalTabPinnedBorderThickness : 0;
	const shouldShareRow =
		input.sizeBounds.width === null ||
		(!input.collapsed && input.sizeBounds.width >= LAYOUT_CONSTANTS.verticalTabMinWidth * 2);
	if (shouldShareRow) {
		let x = 0;
		let height = 0;
		const children = input.children.map((preferred, index) => {
			const first = index === 0;
			let width = preferred.width;
			if (input.sizeBounds.width !== null) {
				width = first
					? Math.floor((input.sizeBounds.width + 2 * border - SPLIT_VIEW_GAP) / 2)
					: input.sizeBounds.width - x;
			}
			const bounds = {
				x,
				y: 0,
				width,
				height: preferred.height,
			};
			x += width - 2 * border + (first ? SPLIT_VIEW_GAP : 0);
			height = Math.max(height, preferred.height);
			return {
				id: `split-${index}`,
				visible: input.visible?.[index] ?? true,
				bounds,
			};
		});
		return {
			hostSize: {
				width: x,
				height,
			},
			children,
		};
	}
	const width = requireDefined(input.sizeBounds.width);
	let y = 0;
	const children = input.children.map((preferred, index) => {
		const bounds = {
			x: 0,
			y,
			width,
			height: preferred.height,
		};
		y += preferred.height - 2 * border + (index === 0 ? SPLIT_VIEW_GAP : 0);
		return {
			id: `split-${index}`,
			visible: input.visible?.[index] ?? true,
			bounds,
		};
	});
	return {
		hostSize: {
			width,
			height: y + 2 * border,
		},
		children,
	};
}
