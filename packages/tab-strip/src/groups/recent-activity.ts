import type { TabGroupActivity } from '../model';

export const RECENT_ACTIVITY_MAX_ROWS = 5;
export const RECENT_ACTIVITY_BUBBLE_CONTENT_MARGIN = 20;

export interface RecentActivitySections {
	tabActivity: TabGroupActivity | null;
	groupActivity: readonly TabGroupActivity[];
	empty: boolean;
}

/**
 * Ports RecentActivityBubbleDialogView's row allocation: show the newest tab
 * event once, then use the remaining capacity (at most five rows) for group
 * activity.
 */
export function getRecentActivitySections(
	activity: readonly Readonly<TabGroupActivity>[],
): RecentActivitySections {
	const tabActivity = activity.find((item) => item.scope === 'tab') ?? null;
	const groupCapacity = RECENT_ACTIVITY_MAX_ROWS - (tabActivity ? 1 : 0);
	const groupActivity = activity.filter((item) => item.scope === 'group').slice(0, groupCapacity);
	return {
		tabActivity,
		groupActivity,
		empty: tabActivity === null && groupActivity.length === 0,
	};
}

export function isRecentActivityActionEnabled(action: TabGroupActivity['action']): boolean {
	return action !== 'none';
}

export function getRecentActivityDescription(item: Readonly<TabGroupActivity>): string {
	return item.description ? `${item.description} • ${item.timeText}` : item.timeText;
}
