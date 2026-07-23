import type { TabData } from '../../../../../model/tab-model';
import {
	resolveTabIconNetworkState,
	tabIconNetworkStateIsAnimated,
} from '../components/tab-icon/utils/tab-icon';

export const DEFAULT_TAB_TITLE = 'New Tab';
export const LOADING_TAB_TITLE = 'Loading…';

/** Visible-label portion of Tab::OnTabDataChanged for the shared TabTitle. */
export function getVisibleTabTitle(tab: Readonly<TabData>): string {
	if (!tab.shouldRenderLoadingTitle) return tab.title || DEFAULT_TAB_TITLE;
	const showingLoadingAnimation =
		!tab.shouldHideThrobber && tabIconNetworkStateIsAnimated(resolveTabIconNetworkState(tab));
	return showingLoadingAnimation ? LOADING_TAB_TITLE : DEFAULT_TAB_TITLE;
}
