// Ported from tabs/shared/tab_strip_types.h and the vertical state controller.
export const TabStripOrientation = {
	Horizontal: 'horizontal',
	Vertical: 'vertical',
} as const;

export type TabStripOrientation = (typeof TabStripOrientation)[keyof typeof TabStripOrientation];

export const ToggleTabGroupCollapsedStateOrigin = {
	MenuAction: 'menu-action',
	Mouse: 'mouse',
	Keyboard: 'keyboard',
	Gesture: 'gesture',
	TabsSelected: 'tabs-selected',
} as const;

export type ToggleTabGroupCollapsedStateOrigin =
	(typeof ToggleTabGroupCollapsedStateOrigin)[keyof typeof ToggleTabGroupCollapsedStateOrigin];

export const VerticalTabStripCollapseState = {
	Expanded: 'expanded',
	Collapsing: 'collapsing',
	Collapsed: 'collapsed',
	Expanding: 'expanding',
} as const;

export type VerticalTabStripCollapseState =
	(typeof VerticalTabStripCollapseState)[keyof typeof VerticalTabStripCollapseState];
