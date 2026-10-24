export { TabStrip } from './TabStrip';
export {
	TAB_STRIP_ACTION_IDS,
	tabCloseInput,
	tabSelectionInput,
	type TabCloseInput,
	type TabSelectionInput,
} from './actions/tab-strip-action-contracts';
export type {
	TabStripCrossWindowDragRuntimeFactory,
	TabStripCloseCallback,
	TabStripCloseConfirmation,
	TabStripMessages,
	TabStripNewTabRequest,
	TabStripTabCreationIntent,
	TabStripSelectCallback,
	TabStripShortcutLabels,
	TabStatusAdornment,
	TabStatusAdornmentKind,
} from './TabStrip';
export { TabStripOrientation } from './types/tab-strip-types';
export { ThumbnailCaptureReadiness } from './components/tab-strip-view/components/hover-card/model/tab-hover-card-controller';
export type {
	HoverCardThumbnailSource,
	HoverCardThumbnailSubscription,
} from './components/tab-strip-view/components/hover-card/model/tab-hover-card-thumbnail-observer';
export {
	TabModel,
	type TabAlertState,
	type TabData,
	type TabGroupData,
	type TabModelInit,
	type TabModelSnapshot,
} from './model/tab-model';
