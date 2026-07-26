import type { Rect } from '../../../../../../../utils/rectRight';
import type { Size } from '../../../../../../../layout/proposed-layout';
import { LAYOUT_CONSTANTS } from '../../../../../../../layout/layout-constants';
import { TabStripOrientation } from '../../../../../../../types/tab-strip-types';

export interface VerticalTopContainerChildLayout {
	id: 'collapse' | 'combo';
	visible: boolean;
	bounds: Rect;
}

export interface VerticalTopContainerLayoutInput {
	availableWidth: number;
	parentAvailableWidth?: number;
	widthIsBounded?: boolean;
	toolbarHeight?: number;
	captionButtonWidth?: number;
	collapsed?: boolean;
	isExitingExpandOnHover?: boolean;
	collapseVisible?: boolean;
	comboEndVisible?: boolean;
	collapsePreferredSize?: Size;
	comboButtonSize?: number;
}

export interface VerticalTopContainerLayout {
	hostSize: Size;
	orientation: TabStripOrientation;
	wrappedDueToOverflow: boolean;
	isPositionInCaption(point: Readonly<{ x: number; y: number }>): boolean;
	children: readonly VerticalTopContainerChildLayout[];
}

function comboPreferredSize(visible: boolean, buttonSize: number): Size {
	if (!visible) return { width: 0, height: 0 };
	return { width: buttonSize, height: buttonSize };
}

function contains(rect: Readonly<Rect>, point: Readonly<{ x: number; y: number }>): boolean {
	return (
		point.x >= rect.x &&
		point.x < rect.x + rect.width &&
		point.y >= rect.y &&
		point.y < rect.y + rect.height
	);
}

export function getVerticalTopContainerPreferredWidth(
	input: VerticalTopContainerLayoutInput,
): number {
	const buttonSize =
		input.comboButtonSize ?? LAYOUT_CONSTANTS.verticalTabStripTopContainerButtonSize;
	const combo = comboPreferredSize(input.comboEndVisible ?? true, buttonSize);
	const padding = LAYOUT_CONSTANTS.verticalTabStripTopButtonPadding;
	let width = combo.width;
	if (input.collapseVisible ?? true)
		width +=
			(input.collapsePreferredSize?.width ?? LAYOUT_CONSTANTS.verticalTabStripCollapseButtonSize) +
			padding;
	return width;
}

export function willVerticalTopContainerWrap(
	input: VerticalTopContainerLayoutInput,
	availableWidth = input.availableWidth,
): boolean {
	const captionWidth = input.captionButtonWidth ?? 0;
	return (
		captionWidth > 0 &&
		getVerticalTopContainerPreferredWidth(input) + captionWidth >= availableWidth
	);
}

/** Direct port of VerticalTabStripTopContainer::CalculateProposedLayout. */
export function calculateVerticalTopContainerLayout(
	input: VerticalTopContainerLayoutInput,
): VerticalTopContainerLayout {
	const toolbarHeight = Math.max(0, input.toolbarHeight ?? 0);
	const captionWidth = Math.max(0, input.captionButtonWidth ?? 0);
	const collapseVisible = input.collapseVisible ?? true;
	const comboVisible = input.comboEndVisible ?? true;
	const collapsed = input.collapsed ?? false;
	const bounded = input.widthIsBounded ?? true;
	const hostWidth = Math.max(0, input.availableWidth);
	const availableWidth = hostWidth > 0 ? hostWidth : Math.max(0, input.parentAvailableWidth ?? 0);
	const collapseSize = input.collapsePreferredSize ?? {
		width: LAYOUT_CONSTANTS.verticalTabStripCollapseButtonSize,
		height: LAYOUT_CONSTANTS.verticalTabStripCollapseButtonSize,
	};
	const comboButtonSize =
		input.comboButtonSize ?? LAYOUT_CONSTANTS.verticalTabStripTopContainerButtonSize;
	const comboHorizontal = comboPreferredSize(comboVisible, comboButtonSize);
	const useVertical =
		comboHorizontal.width >= availableWidth ||
		availableWidth < LAYOUT_CONSTANTS.verticalTabStripCollapseSnapWidth ||
		collapsed;
	const orientation = useVertical ? TabStripOrientation.Vertical : TabStripOrientation.Horizontal;
	const children: VerticalTopContainerChildLayout[] = [];
	let hostHeight = toolbarHeight;
	let computedHostWidth = hostWidth;

	const add = (id: VerticalTopContainerChildLayout['id'], visible: boolean, bounds: Rect) => {
		children.push({ id, visible, bounds });
	};

	const baselineMinHeight = () => {
		let height = 0;
		if (collapseVisible) height = Math.max(height, collapseSize.height);
		if (comboVisible) height = Math.max(height, comboHorizontal.height);
		if (toolbarHeight === 0) height += LAYOUT_CONSTANTS.toolbarButtonInsetHeight;
		return Math.max(toolbarHeight, height);
	};

	const wrappedDueToOverflow =
		!useVertical && bounded && willVerticalTopContainerWrap(input, availableWidth);

	if (useVertical) {
		const firstButtonHeight = collapseVisible ? collapseSize.height : 0;
		let y =
			firstButtonHeight > 0
				? Math.max(0, Math.floor((baselineMinHeight() - firstButtonHeight) / 2))
				: 0;
		if (captionWidth > 0) y = Math.max(y, toolbarHeight);

		if (collapseVisible) {
			const x = collapsed
				? 0
				: Math.max(0, Math.floor((computedHostWidth - collapseSize.width) / 2));
			const bounds = { x, y, ...collapseSize };
			add('collapse', true, bounds);
			computedHostWidth = Math.max(computedHostWidth, x + collapseSize.width);
			y += collapseSize.height;
		}
		if (comboVisible) {
			y += LAYOUT_CONSTANTS.verticalTabStripCollapsedVerticalPadding;
			const preferred = comboPreferredSize(comboVisible, comboButtonSize);
			const padding = collapsed
				? LAYOUT_CONSTANTS.verticalTabStripFlatEdgeButtonPadding
				: Math.max(0, Math.floor((computedHostWidth - preferred.width) / 2));
			const width = collapsed
				? Math.max(
						0,
						computedHostWidth - 2 * LAYOUT_CONSTANTS.verticalTabStripFlatEdgeButtonPadding,
					)
				: preferred.width;
			const bounds = { x: padding, y, width, height: preferred.height };
			add('combo', true, bounds);
			computedHostWidth = Math.max(computedHostWidth, bounds.x + bounds.width);
			y += preferred.height + LAYOUT_CONSTANTS.verticalTabStripCollapsedVerticalPadding;
		} else if (captionWidth !== 0) {
			y += LAYOUT_CONSTANTS.verticalTabStripCollapsedVerticalPadding;
		}
		hostHeight = Math.max(hostHeight, y);
	} else {
		hostHeight = Math.max(hostHeight, baselineMinHeight());
		const left = wrappedDueToOverflow || input.isExitingExpandOnHover ? 0 : captionWidth;
		let yBaseline = Math.floor(hostHeight / 2);
		if (wrappedDueToOverflow) {
			hostHeight += LAYOUT_CONSTANTS.bookmarkBarHeight;
			yBaseline =
				toolbarHeight +
				Math.floor(
					(LAYOUT_CONSTANTS.bookmarkBarHeight -
						LAYOUT_CONSTANTS.bookmarkBarButtonImageLabelPadding) /
						2,
				);
		}
		let nextComboY = 0;
		if (collapseVisible) {
			const bounds = {
				x: left,
				y: Math.max(0, yBaseline - Math.floor(collapseSize.height / 2)),
				...collapseSize,
			};
			add('collapse', true, bounds);
			nextComboY =
				bounds.y + bounds.height + LAYOUT_CONSTANTS.verticalTabStripCollapsedVerticalPadding;
		}
		if (comboVisible) {
			const wrapDuringAnimation =
				captionWidth !== 0 && availableWidth < getVerticalTopContainerPreferredWidth(input);
			const bounds = {
				x: wrapDuringAnimation ? 0 : computedHostWidth - comboHorizontal.width,
				y: wrapDuringAnimation
					? nextComboY
					: Math.max(0, yBaseline - Math.floor(comboHorizontal.height / 2)),
				width: comboHorizontal.width,
				height: comboHorizontal.height,
			};
			add('combo', true, bounds);
		}
	}

	return {
		hostSize: { width: computedHostWidth, height: hostHeight },
		orientation,
		wrappedDueToOverflow,
		children,
		isPositionInCaption: (point) =>
			!children.some((child) => child.visible && contains(child.bounds, point)),
	};
}
