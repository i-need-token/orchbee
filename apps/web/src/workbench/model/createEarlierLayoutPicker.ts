import { workbenchMessages, type IntlShape } from '@orchbee/i18n';

import type { WorkbenchEarlierLayoutInput } from '../actions/workbench-action-ids';
import type { WorkbenchEarlierLayout, WorkbenchEarlierLayouts } from '../types/workbench-authority';
import type { WorkbenchSearchProviderArgumentPicker } from '../types/workbench-search-provider';

const MINUTE = 60_000;

interface CreateEarlierLayoutPickerOptions {
	readonly earlierLayouts: WorkbenchEarlierLayouts;
	readonly intl: IntlShape;
	/** Unix milliseconds now; relative times are read against it. */
	readonly now?: () => number;
}

function twoDigits(value: number): string {
	return String(value).padStart(2, '0');
}

/** How long ago a layout was kept, in the TUI's steps. */
function relativeTime(intl: IntlShape, minutes: number): string {
	if (minutes === 0) return intl.formatMessage(workbenchMessages.earlierLayoutJustNow);
	if (minutes < 60) {
		return intl.formatMessage(workbenchMessages.earlierLayoutMinutesAgo, { count: minutes });
	}
	if (minutes < 2_880) {
		return intl.formatMessage(workbenchMessages.earlierLayoutHoursAgo, {
			count: Math.floor(minutes / 60),
		});
	}
	return intl.formatMessage(workbenchMessages.earlierLayoutDaysAgo, {
		count: Math.floor(minutes / 1_440),
	});
}

/**
 * When a layout was kept, in the TUI's words: relative time, then the local clock, with the date
 * once the layout is from another day.
 */
function formatWhen(intl: IntlShape, savedAt: number, now: number): string {
	const relative = relativeTime(intl, Math.floor(Math.max(0, now - savedAt) / MINUTE));
	const saved = new Date(savedAt);
	const today = new Date(now);
	const time = `${twoDigits(saved.getHours())}:${twoDigits(saved.getMinutes())}`;
	const sameDay =
		saved.getFullYear() === today.getFullYear() &&
		saved.getMonth() === today.getMonth() &&
		saved.getDate() === today.getDate();
	const clock = sameDay
		? time
		: `${saved.getFullYear()}-${twoDigits(saved.getMonth() + 1)}-${twoDigits(saved.getDate())} ${time}`;
	return intl.formatMessage(workbenchMessages.earlierLayoutWhenClock, { clock, relative });
}

/** What a layout holds that is not open now, or that everything in it is open. */
function formatSummary(intl: IntlShape, layout: WorkbenchEarlierLayout): string {
	const { tabsNotOpen: tabs, workspacesNotOpen: workspaces } = layout;
	if (workspaces === 0 && tabs === 0) {
		return intl.formatMessage(workbenchMessages.earlierLayoutAllOpen);
	}
	return workspaces === 0
		? intl.formatMessage(workbenchMessages.earlierLayoutTabsNotOpen, { tabs })
		: intl.formatMessage(workbenchMessages.earlierLayoutWorkspacesTabsNotOpen, {
				tabs,
				workspaces,
			});
}

/**
 * Search's choice for Earlier layouts…: the layouts the Server kept, newest first, read each time
 * the picker opens. Each row says when the layout was kept and what of it is not open now, as the
 * TUI's picker rows do; no layout, or a listing that fails, shows one disabled row saying why.
 */
export function createEarlierLayoutPicker({
	earlierLayouts,
	intl,
	now = Date.now,
}: CreateEarlierLayoutPickerOptions): WorkbenchSearchProviderArgumentPicker {
	const unavailable = intl.formatMessage(workbenchMessages.earlierLayoutsUnavailable);
	return {
		kind: 'read',
		placeholder: intl.formatMessage(workbenchMessages.searchArgumentPlaceholder),
		prompt: intl.formatMessage(workbenchMessages.searchArgumentPrompt, {
			action: intl.formatMessage(workbenchMessages.earlierLayouts),
		}),
		read: async () => {
			const listing = await earlierLayouts.list();
			if (listing.kind === 'unavailable') return { kind: 'unavailable', label: unavailable };
			if (listing.layouts.length === 0) {
				return {
					kind: 'unavailable',
					label: intl.formatMessage(workbenchMessages.earlierLayoutsEmpty),
				};
			}
			const readAt = now();
			return {
				kind: 'options',
				options: listing.layouts.map((layout, index) => {
					const input: WorkbenchEarlierLayoutInput = { savedAt: layout.savedAt };
					const label = formatWhen(intl, layout.savedAt, readAt);
					const description = formatSummary(intl, layout);
					return {
						description,
						// Newest first, kept when a query sorts the matches by identity.
						id: String(index).padStart(4, '0'),
						input,
						keywords: [description],
						label,
					};
				}),
			};
		},
		readFailed: unavailable,
		reading: intl.formatMessage(workbenchMessages.earlierLayoutsReading),
	};
}
