import { requireDefined } from '../utils/requireDefined';
import type { TabData, TabGroupData, TabAlertState } from '../model/tab-model';
import { getTabAlertAccessibleDescription } from '../components/tab-strip-view/components/tab-button/components/alert-indicator-button/model/alert-indicator-button';
const GROUP_CONTENT_TITLE_MAX_LENGTH = 30;
const GROUP_HOVER_ITEM_MAX_LENGTH = 50;
const GROUP_HOVER_BULLET = '•  ';
const GROUP_HOVER_CARD_MAX_TABS = 5;
function truncate(value: string, maximumLength: number): string {
	const segments =
		typeof Intl.Segmenter === 'function'
			? [
					...new Intl.Segmenter(undefined, {
						granularity: 'grapheme',
					}).segment(value),
				].map((part) => part.segment)
			: Array.from(value);
	if (segments.length <= maximumLength) return value;
	return `${segments
		.slice(0, Math.max(0, maximumLength - 1))
		.join('')
		.trimEnd()}…`;
}
function formatBytes(bytes: number): string {
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
function alertDescription(alertState: TabAlertState | undefined): string | null {
	return alertState ? getTabAlertAccessibleDescription(alertState) : null;
}

/** Port of GetGroupContentString: first title, then the remaining tab count. */
function getGroupContentString(tabs: readonly Pick<TabData, 'title'>[]): string {
	const firstTitle = truncate(tabs[0]?.title || 'New Tab', GROUP_CONTENT_TITLE_MAX_LENGTH);
	const remaining = Math.max(0, tabs.length - 1);
	if (remaining === 0) return firstTitle;
	return `${firstTitle} and ${remaining} other ${remaining === 1 ? 'tab' : 'tabs'}`;
}

/** Fallback tooltip used when Chromium's tab-group hover-card feature is off. */
export function getGroupTooltipText(
	group: Pick<TabGroupData, 'title'>,
	tabs: readonly Pick<TabData, 'title'>[],
): string {
	const contents = getGroupContentString(tabs);
	return group.title ? `${group.title} - ${contents}` : contents;
}

/**
 * Port of GetHoverCardAccessibilityText. Chromium exposes the count, at most
 * five character-truncated titles, the overflow count, and state.
 */
export function getGroupAccessibleName(
	group: Readonly<TabGroupData>,
	tabs: readonly Pick<TabData, 'title'>[],
): string {
	const parts: string[] = [];
	parts.push(group.title || 'Unnamed group');
	parts.push(`${tabs.length} ${tabs.length === 1 ? 'tab' : 'tabs'}`);
	tabs.slice(0, GROUP_HOVER_CARD_MAX_TABS).forEach((tab) => {
		parts.push(
			`${GROUP_HOVER_BULLET}${truncate(tab.title || 'New Tab', GROUP_HOVER_ITEM_MAX_LENGTH - GROUP_HOVER_BULLET.length)}`,
		);
	});
	if (tabs.length > GROUP_HOVER_CARD_MAX_TABS) {
		const excess = tabs.length - GROUP_HOVER_CARD_MAX_TABS;
		parts.push(`+ ${excess} more`);
	}
	parts.push(group.collapsed ? 'collapsed' : 'expanded');
	if (group.collapsed && group.attention && tabs.length > 0) parts.push('new activity');
	return parts.join(', ');
}
interface AccessibleTabContext {
	group?: Readonly<TabGroupData> | null;
}

/** Port of tabs::GetAccessibleTabLabel, preserving Chromium's wrapper order. */
export function getAccessibleTabLabel(
	tab: Readonly<TabData>,
	context: AccessibleTabContext = {},
): string {
	let title = tab.title || 'New Tab';
	if (tab.group !== null) {
		title = context.group?.title
			? `${title}, in group ${context.group.title}`
			: `${title}, in unnamed group`;
	}
	if (tab.pinned) title = `${title}, pinned`;

	const alert = alertDescription(tab.alertState);
	if (alert) title = `${title}, ${alert}`;
	if (tab.discarded) {
		title = `${title}, inactive tab`;
		if ((tab.discardedMemorySavingsBytes ?? 0) > 0) {
			title = `${title}, ${formatBytes(requireDefined(tab.discardedMemorySavingsBytes))} memory saved`;
		}
	} else if ((tab.memoryUsageBytes ?? 0) > 0) {
		title = `${title}, ${tab.memoryUsageHigh ? 'high memory usage' : 'memory usage'} ${formatBytes(requireDefined(tab.memoryUsageBytes))}`;
	}
	return title;
}
export type TabMoveDirection = 'first' | 'last' | 'previous' | 'next';
export function getTabMoveAnnouncement(
	direction: TabMoveDirection,
	orientation: 'horizontal' | 'vertical',
): string {
	if (direction === 'first') return 'Tab moved to first position';
	if (direction === 'last') return 'Tab moved to last position';
	if (orientation === 'vertical')
		return direction === 'previous' ? 'Tab moved up' : 'Tab moved down';
	return direction === 'previous' ? 'Tab moved left' : 'Tab moved right';
}
export function getTabGroupMembershipAnnouncement(
	action: 'added' | 'removed',
	group: Readonly<TabGroupData>,
	tabsBeforeChange: readonly Pick<TabData, 'title'>[],
): string {
	const target = group.title
		? `group ${group.title}, ${getGroupContentString(tabsBeforeChange)}`
		: `unnamed group, ${getGroupContentString(tabsBeforeChange)}`;
	return `Tab ${action === 'added' ? 'added to' : 'removed from'} ${target}`;
}
