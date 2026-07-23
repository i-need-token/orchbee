// Ported from tabs/shared/tab_strip_types.h and the vertical state controller.
export const TabStripOrientation = {
	Horizontal: 'horizontal',
	Vertical: 'vertical',
} as const;

export type TabStripOrientation = (typeof TabStripOrientation)[keyof typeof TabStripOrientation];

export const EndDragReason = {
	Complete: 'complete',
	Cancel: 'cancel',
	CaptureLost: 'capture-lost',
	ModelAddedTab: 'model-added-tab',
} as const;

export type EndDragReason = (typeof EndDragReason)[keyof typeof EndDragReason];

export const CloseTabSource = {
	FromMouse: 'from-mouse',
	FromTouch: 'from-touch',
	FromNonUIEvent: 'from-non-ui-event',
} as const;

export type CloseTabSource = (typeof CloseTabSource)[keyof typeof CloseTabSource];

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
