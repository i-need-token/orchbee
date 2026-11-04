export { TabStrip } from './TabStrip';
export {
	TAB_STRIP_ACTION_IDS,
	type TabCloseInput,
	type TabSelectionInput,
} from './actions/tab-strip-action-contracts';
export type {
	TabStripArrangementChange,
	TabStripChangeCallback,
	TabStripCloseCallback,
	TabStripCloseConfirmation,
	TabStripCloseResume,
	TabStripGroupNameRequest,
	TabStripMessages,
	TabStripNewTabRequest,
	TabStripTabCreationIntent,
	TabStripSelectCallback,
	TabStripNewTabChoice,
	TabStatusAdornment,
	TabStatusAdornmentKind,
} from './TabStrip';
export { TabStripOrientation } from './types/tab-strip-types';
export { TabModel, type TabData } from './model/tab-model';
export { getTabCloseTargets } from './model/tab-close-targets';
