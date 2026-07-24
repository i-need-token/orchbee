export const TAB_STYLE_MAXIMUM_Z_VALUE = 7;
export const TAB_Z_VALUE_RESERVED_BITS = 3;
export const ACTIVE_TAB_Z_VALUE_BIT = 1 << (TAB_Z_VALUE_RESERVED_BITS + 1);
export const GROUP_VIEW_Z_VALUE_BIT = 1 << TAB_Z_VALUE_RESERVED_BITS;

export type TabContainerElementKind =
	| 'tab'
	| 'group-header'
	| 'group-underline'
	| 'group-highlight';

export interface TabContainerZValueInput {
	kind: TabContainerElementKind;
	active?: boolean;
	selected?: boolean;
	hovering?: boolean;
	hoverAnimationValue?: number;
}

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

/** Ports TabStyleViewsImpl::GetZValue(). */
export function getTabStyleZValue(input: {
	selected?: boolean;
	hovering?: boolean;
	hoverAnimationValue?: number;
}): number {
	let value = clamp01(input.hoverAnimationValue ?? 0);
	if (input.selected) value += 4;
	if (input.hovering) value += 2;
	return value;
}

/** Ports ZOrderableTabContainerElement::CalculateZValue(). */
export function calculateTabContainerZValue(input: Readonly<TabContainerZValueInput>): number {
	if (input.kind === 'group-highlight') return 0;
	if (input.kind === 'group-header') {
		return GROUP_VIEW_Z_VALUE_BIT + (input.active ? ACTIVE_TAB_Z_VALUE_BIT : 0);
	}
	if (input.kind === 'group-underline') {
		return GROUP_VIEW_Z_VALUE_BIT + ACTIVE_TAB_Z_VALUE_BIT + 1;
	}
	const active = input.active ? ACTIVE_TAB_Z_VALUE_BIT : 0;
	return active + getTabStyleZValue(input) + 1;
}

export function compareTabContainerZValues(
	left: Readonly<TabContainerZValueInput>,
	right: Readonly<TabContainerZValueInput>,
): number {
	return calculateTabContainerZValue(left) - calculateTabContainerZValue(right);
}
