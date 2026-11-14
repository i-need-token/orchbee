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
	TabStripGroupNameRequest,
	TabStripMessages,
	TabStripNewTabRequest,
	TabStripTabCreationIntent,
	TabStripSelectCallback,
	TabStripNewTabChoice,
	TabStripNewTabSection,
	TabStatusAdornment,
	TabStatusAdornmentKind,
} from './TabStrip';
export { TabStripOrientation } from './types/tab-strip-types';
export { TabModel, type TabData } from './model/tab-model';
export { getTabCloseTargets } from './model/tab-close-targets';
// A host that draws one Tab outside the strip draws its icon and title as the strip does.
export { TabIcon } from './components/tab-strip-view/components/tab-button/components/tab-icon/TabIcon';
export { getVisibleTabTitle } from './components/tab-strip-view/components/tab-button/utils/getVisibleTabTitle';
// A host that offers the New Tab menu outside the strip draws its sections as the strip does.
export { TabStripNewTabMenuItems } from './components/tab-strip-view/components/tab-strip-context-menu/components/tab-strip-new-tab-menu-items/TabStripNewTabMenuItems';
