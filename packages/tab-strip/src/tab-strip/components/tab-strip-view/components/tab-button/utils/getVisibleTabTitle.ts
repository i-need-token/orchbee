import { DEFAULT_TAB_STRIP_MESSAGES } from '../../../../../constants/default-tab-strip-messages';
import type { TabData } from '../../../../../model/tab-model';
import type { TabStripMessages } from '../../../../../types/tab-strip-messages';
import {
	resolveTabIconNetworkState,
	tabIconNetworkStateIsAnimated,
} from '../components/tab-icon/utils/tab-icon';

/** Visible-label portion of Tab::OnTabDataChanged for the shared TabTitle. */
export function getVisibleTabTitle(
	tab: Readonly<TabData>,
	messages: Pick<TabStripMessages, 'loadingTabTitle' | 'newTabTitle'> = DEFAULT_TAB_STRIP_MESSAGES,
): string {
	if (!tab.shouldRenderLoadingTitle) return tab.title || messages.newTabTitle;
	const showingLoadingAnimation =
		!tab.shouldHideThrobber && tabIconNetworkStateIsAnimated(resolveTabIconNetworkState(tab));
	return showingLoadingAnimation
		? (messages.loadingTabTitle ?? DEFAULT_TAB_STRIP_MESSAGES.loadingTabTitle)
		: messages.newTabTitle;
}
