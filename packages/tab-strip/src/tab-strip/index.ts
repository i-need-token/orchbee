export { TabStrip } from './TabStrip';
export type {
	TabStripCrossWindowDragRuntime,
	TabStripCrossWindowDragRuntimeFactory,
	TabStripMessages,
	TabStripNewTabRequest,
	TabStripProps,
} from './TabStrip';
export { TabStripOrientation, VerticalTabStripCollapseState } from './types/tab-strip-types';
export {
	TabHoverCardController,
	ThumbnailCaptureReadiness,
} from './components/tab-strip-view/components/hover-card/model/tab-hover-card-controller';
export type {
	HoverCardThumbnailSource,
	HoverCardThumbnailSubscription,
} from './components/tab-strip-view/components/hover-card/model/tab-hover-card-thumbnail-observer';
export {
	TabModel,
	type TabAlertState,
	type TabData,
	type DetachedTabBundle,
	type TabGroupColor,
	type TabGroupData,
	type TabModelInit,
	type TabModelSnapshot,
	type TabDragRestoreState,
} from './model/tab-model';
export {
	CrossWindowDragState,
	CrossWindowTabDragRuntime,
	type CrossWindowDragEndReason,
	type CrossWindowDragHost,
	type CrossWindowDragSnapshot,
} from './dragging/cross-window-drag-runtime';
export {
	DomTabDragRegistry,
	MOUSE_VERTICAL_DETACH_MAGNETISM,
	TOUCH_VERTICAL_DETACH_MAGNETISM,
	domRectToScreenRect,
	type DomTabDragBrowserRegistration,
	type DomTabDragRegistryOptions,
} from './dragging/dom-tab-drag-registry';
export {
	DragSessionData,
	DragSlotViewType,
	type DragSessionDataInit,
	type GroupHeaderDragData,
	type TabDragData,
	type TabDragGroupData,
} from './dragging/drag-session-data';
export type { DragPoint } from './dragging/dragging-tabs-session';
export type {
	TabDragContext,
	TabDragPositioningDelegate,
	TabDragPositioningDelegateBase,
} from './dragging/tab-drag-context';
export type {
	TabDragPointResolver,
	TabDragTarget,
	TabDragTargetController,
} from './dragging/tab-drag-target';
export { TabStripComboButtonController } from './model/tab-strip-combo-button';
export { VerticalTabStripExpandOnHoverController } from './model/vertical-tab-strip-expand-on-hover';
