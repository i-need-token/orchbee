import type { Rect } from '../utils/rectRight';
import { TabStripOrientation } from '../types/tab-strip-types';
import { LAYOUT_CONSTANTS } from './layout-constants';
import type { Size, SizeBounds } from './proposed-layout';

export interface TabStripRegionLayout {
	hostSize: Size;
	pinnedBounds: Rect;
	pinnedVisible: boolean;
	separatorBounds: Rect;
	separatorVisible: boolean;
	unpinnedBounds: Rect;
	unpinnedVisible: boolean;
}

export interface TabStripViewLayoutInput {
	orientation: TabStripOrientation;
	sizeBounds: SizeBounds;
	pinnedPreferred: Size;
	pinnedMinimum: Size;
	pinnedVisible?: boolean;
	unpinnedPreferred: Size;
	unpinnedMinimum: Size;
	unpinnedVisible?: boolean;
	separatorPreferredHeight?: number;
	collapsed?: boolean;
}

/** Port of the helper in common/tab_strip_view_layout.cc. */
export function calculatePinnedContainerMainAxisSize(
	pinnedPreferred: number,
	unpinnedPreferred: number,
	available: number,
	pinnedMinimum: number,
): number {
	return Math.max(
		Math.min(pinnedPreferred, Math.max(Math.floor(available / 2), available - unpinnedPreferred)),
		pinnedMinimum,
	);
}

export function calculateTabStripViewLayout(input: TabStripViewLayoutInput): TabStripRegionLayout {
	return input.orientation === TabStripOrientation.Horizontal
		? calculateHorizontal(input)
		: calculateVertical(input);
}

function calculateHorizontal(input: TabStripViewLayoutInput): TabStripRegionLayout {
	const containerHeight = input.sizeBounds.height ?? LAYOUT_CONSTANTS.tabHeight;
	let pinnedWidth = input.pinnedPreferred.width;
	if (input.sizeBounds.width !== null) {
		pinnedWidth = calculatePinnedContainerMainAxisSize(
			input.pinnedPreferred.width,
			input.unpinnedPreferred.width,
			input.sizeBounds.width,
			input.pinnedMinimum.width,
		);
	}

	const x = pinnedWidth > 0 ? pinnedWidth : 0;
	let unpinnedWidth = input.unpinnedPreferred.width;
	if (input.sizeBounds.width !== null) {
		unpinnedWidth = Math.max(
			Math.min(input.unpinnedPreferred.width, Math.max(input.sizeBounds.width - pinnedWidth, 0)),
			input.unpinnedMinimum.width,
		);
	}

	return {
		hostSize: { width: x + unpinnedWidth, height: containerHeight },
		pinnedBounds: { x: 0, y: 0, width: pinnedWidth, height: containerHeight },
		pinnedVisible: input.pinnedVisible ?? true,
		separatorBounds: { x: 0, y: 0, width: 0, height: 0 },
		separatorVisible: false,
		unpinnedBounds: { x, y: 0, width: unpinnedWidth, height: containerHeight },
		unpinnedVisible: input.unpinnedVisible ?? true,
	};
}

function calculateVertical(input: TabStripViewLayoutInput): TabStripRegionLayout {
	const width = input.sizeBounds.width;
	if (width === null) {
		return {
			hostSize: { width: 0, height: 0 },
			pinnedBounds: { x: 0, y: 0, width: 0, height: 0 },
			pinnedVisible: input.pinnedVisible ?? true,
			separatorBounds: { x: 0, y: 0, width: 0, height: 0 },
			separatorVisible: false,
			unpinnedBounds: { x: 0, y: 0, width: 0, height: 0 },
			unpinnedVisible: input.unpinnedVisible ?? true,
		};
	}

	const horizontalPadding = LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding;
	const verticalPadding = LAYOUT_CONSTANTS.verticalTabStripCollapsedVerticalPadding;
	const pinnedPreferredHeight = input.pinnedPreferred.height;
	const unpinnedPreferredHeight = input.unpinnedPreferred.height;
	const separatorHeight = input.separatorPreferredHeight ?? 1;
	const separatorVisible =
		pinnedPreferredHeight !== 0 && unpinnedPreferredHeight !== 0 && Boolean(input.collapsed);

	let remainingHeight = 0;
	if (input.sizeBounds.height !== null) {
		remainingHeight = input.sizeBounds.height;
		if (pinnedPreferredHeight !== 0 && unpinnedPreferredHeight !== 0) {
			remainingHeight -= verticalPadding;
		}
		if (separatorVisible) {
			remainingHeight -= separatorHeight + verticalPadding;
		}
		remainingHeight = Math.max(remainingHeight, 0);
	}

	let y = 0;
	let pinnedHeight = pinnedPreferredHeight;
	if (input.sizeBounds.height !== null) {
		pinnedHeight = calculatePinnedContainerMainAxisSize(
			pinnedPreferredHeight,
			unpinnedPreferredHeight,
			remainingHeight,
			input.pinnedMinimum.height,
		);
		remainingHeight -= pinnedHeight;
	}
	const pinnedBounds = {
		x: horizontalPadding,
		y,
		width: width - horizontalPadding,
		height: pinnedHeight,
	};

	if (pinnedHeight !== 0) {
		y += pinnedHeight;
		if (unpinnedPreferredHeight !== 0) {
			y += verticalPadding;
		}
	}

	let separatorBounds: Rect = { x: 0, y: 0, width: 0, height: 0 };
	if (separatorVisible) {
		separatorBounds = {
			x: horizontalPadding,
			y,
			width: width - 2 * horizontalPadding,
			height: separatorHeight,
		};
		y += separatorHeight + verticalPadding;
	}

	let unpinnedHeight = unpinnedPreferredHeight;
	if (input.sizeBounds.height !== null) {
		unpinnedHeight = Math.max(
			Math.min(unpinnedPreferredHeight, remainingHeight),
			input.unpinnedMinimum.height,
		);
	}
	const unpinnedBounds = { x: 0, y, width, height: unpinnedHeight };

	return {
		hostSize: { width, height: y + unpinnedHeight },
		pinnedBounds,
		pinnedVisible: input.pinnedVisible ?? true,
		separatorBounds,
		separatorVisible,
		unpinnedBounds,
		unpinnedVisible: input.unpinnedVisible ?? true,
	};
}
