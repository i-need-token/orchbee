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
// A host that draws one Tab outside the strip draws its icon and title as the strip does.
export { TabIcon } from './components/tab-strip-view/components/tab-button/components/tab-icon/TabIcon';
export { getVisibleTabTitle } from './components/tab-strip-view/components/tab-button/utils/getVisibleTabTitle';
