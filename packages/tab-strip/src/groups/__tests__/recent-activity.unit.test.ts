import { describe, expect, it } from 'vitest';
import {
	RECENT_ACTIVITY_BUBBLE_CONTENT_MARGIN,
	RECENT_ACTIVITY_MAX_ROWS,
	getRecentActivityDescription,
	getRecentActivitySections,
	isRecentActivityActionEnabled,
} from '../recent-activity';

const item = (id: string, scope: 'tab' | 'group') => ({
	id,
	scope,
	title: id,
	description: 'Alex',
	timeText: '2m',
	action: 'none' as const,
});

describe('recent activity bubble model', () => {
	it('matches the five-row cap and 20dp content margin', () => {
		expect(RECENT_ACTIVITY_MAX_ROWS).toBe(5);
		expect(RECENT_ACTIVITY_BUBBLE_CONTENT_MARGIN).toBe(20);
	});

	it('reserves one row for the newest tab event', () => {
		const sections = getRecentActivitySections([
			item('tab-1', 'tab'),
			item('tab-2', 'tab'),
			...Array.from({ length: 7 }, (_, index) => item(`group-${index}`, 'group')),
		]);
		expect(sections.tabActivity?.id).toBe('tab-1');
		expect(sections.groupActivity).toHaveLength(4);
		expect(sections.empty).toBe(false);
	});

	it('allows five group rows when there is no tab activity', () => {
		expect(
			getRecentActivitySections(Array.from({ length: 7 }, (_, index) => item(`${index}`, 'group')))
				.groupActivity,
		).toHaveLength(5);
		expect(getRecentActivitySections([]).empty).toBe(true);
	});

	it('formats the bullet only when a description exists and disables no-op rows', () => {
		expect(getRecentActivityDescription(item('a', 'group'))).toBe('Alex • 2m');
		expect(getRecentActivityDescription({ ...item('a', 'group'), description: '' })).toBe('2m');
		expect(isRecentActivityActionEnabled('none')).toBe(false);
		expect(isRecentActivityActionEnabled('focus-tab')).toBe(true);
	});
});
