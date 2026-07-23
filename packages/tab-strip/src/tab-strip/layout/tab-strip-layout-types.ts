/** Port of TabSizeInfo from tab_strip_layout_types.h. */
export interface TabSizeInfo {
	pinnedTabWidth: number;
	minActiveWidth: number;
	minInactiveWidth: number;
	standardWidth: number;
}

/** Port of LayoutDomain from tab_strip_layout_types.h. */
export const LayoutDomain = {
	InactiveWidthBelowActiveWidth: 'inactive-width-below-active-width',
	InactiveWidthEqualsActiveWidth: 'inactive-width-equals-active-width',
} as const;

export type LayoutDomain = (typeof LayoutDomain)[keyof typeof LayoutDomain];
