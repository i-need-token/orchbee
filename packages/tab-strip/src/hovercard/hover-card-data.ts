import { requireDefined } from '../common';
import type { TabData, TabGroupData, TabAlertState } from '../model';
import { getTabAlertColorRole, getTabAlertStateText, type TabAlertColorRole } from '../tab';
import { findImageDimensions } from './filename-elider';
export interface HoverCardTitleData {
	text: string;
	isFilename: boolean;
}
export const HOVER_CARD_GROUP_MAX_TABS = 5;
export interface GroupHoverCardData {
	title: HoverCardTitleData;
	tabTitles: readonly HoverCardTitleData[];
	excessText: string;
}
export type HoverCardFooterRow =
	| Readonly<{
			kind: 'alert';
			text: string;
			alertState: TabAlertState;
			colorRole: TabAlertColorRole;
	  }>
	| Readonly<{
			kind: 'discard';
			text: string;
	  }>
	| Readonly<{
			kind: 'performance';
			text: string;
			high: boolean;
	  }>
	| Readonly<{
			kind: 'collaboration';
			text: string;
			avatarUrl?: string;
	  }>;
export function formatHoverCardBytes(bytes: number): string {
	const units = ['bytes', 'KB', 'MB', 'GB', 'TB'];
	let value = Math.max(0, bytes);
	let unit = 0;
	while (value >= 1024 && unit < units.length - 1) {
		value /= 1024;
		unit += 1;
	}
	const digits = unit === 0 || value >= 10 || Number.isInteger(value) ? 0 : 1;
	return `${value.toFixed(digits)} ${units[unit]}`;
}

/** Port of FooterView's independent alert, performance, and collaboration rows. */
export function getHoverCardFooterRows(
	tab: Readonly<TabData>,
	options: Readonly<{
		showMemoryUsage?: boolean;
	}> = {},
): readonly HoverCardFooterRow[] {
	const rows: HoverCardFooterRow[] = [];
	if (tab.discarded) {
		const saved = tab.discardedMemorySavingsBytes ?? 0;
		rows.push({
			kind: 'discard',
			text:
				saved > 0 ? `Inactive tab, ${formatHoverCardBytes(saved)} memory saved` : 'Inactive tab',
		});
	} else if (tab.alertState && tab.alertState !== 'none') {
		rows.push({
			kind: 'alert',
			text: getTabAlertStateText(tab.alertState),
			alertState: tab.alertState,
			colorRole: getTabAlertColorRole(tab.alertState),
		});
	}
	if (
		(tab.memoryUsageBytes ?? 0) > 0 &&
		((options.showMemoryUsage ?? true) || tab.memoryUsageHigh === true) &&
		!tab.discarded &&
		!tab.collaborationMessage
	) {
		const high = tab.memoryUsageHigh === true;
		rows.push({
			kind: 'performance',
			high,
			text: `${high ? 'High memory usage' : 'Memory usage'}: ${formatHoverCardBytes(requireDefined(tab.memoryUsageBytes))}`,
		});
	}
	if (tab.collaborationMessage) {
		rows.push({
			kind: 'collaboration',
			text: `${tab.collaborationMessage.givenName} ${tab.collaborationMessage.event === 'tab-added' ? 'added' : 'changed'} this tab`,
			...(tab.collaborationMessage.avatarUrl === undefined
				? {}
				: {
						avatarUrl: tab.collaborationMessage.avatarUrl,
					}),
		});
	}
	return rows;
}

/** Port of HoverCardAnchorTarget's title filename/image-dimension decision. */
export function getHoverCardTitleData(title: string, url?: string): HoverCardTitleData {
	let isFile = false;
	try {
		isFile = url !== undefined && new URL(url).protocol === 'file:';
	} catch {
		isFile = url?.startsWith('file:') ?? false;
	}
	return {
		text: title,
		isFilename: isFile || findImageDimensions(title) !== null,
	};
}

/** Direct data port of HoverCardAnchorTarget::SetHoverCardDataFrom(TabGroupData). */
export function getGroupHoverCardData(
	group: Readonly<TabGroupData>,
	tabs: readonly Readonly<TabData>[],
): GroupHoverCardData {
	const count = tabs.length;
	const countLabel = `${count} ${count === 1 ? 'tab' : 'tabs'}`;
	const shownTabs = tabs.slice(0, HOVER_CARD_GROUP_MAX_TABS);
	const excess = Math.max(0, count - shownTabs.length);
	return {
		title: {
			text: group.title ? `${group.title} · ${countLabel}` : `${countLabel} in unnamed group`,
			isFilename: false,
		},
		tabTitles: shownTabs.map((tab) => getHoverCardTitleData(tab.title || 'New Tab', tab.url)),
		excessText: excess > 0 ? `+ ${excess} more` : '',
	};
}

/** Port of the domain/source label selection in SetHoverCardDataFrom. */
export function formatHoverCardDomain(url: string | undefined, shouldDisplayUrl = true): string {
	if (!url || !shouldDisplayUrl) return '';
	if (url.startsWith('view-source:')) return 'View source';
	let parsed: URL;
	try {
		parsed = new URL(url);
	} catch {
		return url;
	}
	if (parsed.protocol === 'file:') return 'Local file';
	if (parsed.protocol === 'blob:') return 'Blob';
	if (parsed.href === 'about:blank') return 'about:blank';
	if (parsed.protocol === 'http:' || parsed.protocol === 'https:') {
		return parsed.hostname.replace(/^www\./i, '');
	}
	if (parsed.protocol === 'chrome:' || parsed.protocol === 'edge:') {
		return `${parsed.protocol}//${parsed.host}${parsed.pathname}`;
	}
	return parsed.origin === 'null' ? url : parsed.origin;
}
