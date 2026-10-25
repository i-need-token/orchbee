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
export {
	TabModel,
	type TabData,
	type TabModelInit,
	type TabModelSnapshot,
} from './model/tab-model';
