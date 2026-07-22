import { requireDefined } from '../../common';
import { describe, expect, it } from 'vitest';
import type { TabData, TabGroupData } from '../../model';
import {
	getAccessibleTabLabel,
	getGroupAccessibleName,
	getGroupContentString,
	getGroupTooltipText,
	getTabGroupMembershipAnnouncement,
	getTabMoveAnnouncement,
} from '../tab-strip-accessibility';
const group: TabGroupData = {
	id: 'work',
	title: 'Work',
	color: 'blue',
	collapsed: false,
	shared: true,
};
function tab(update: Partial<TabData> = {}): TabData {
	return {
		id: 'one',
		title: 'Document',
		pinned: false,
		group: null,
		split: null,
		...update,
	};
}
describe('tab strip accessibility ports', () => {
	it('describes group contents with Chromium title limits and pluralization', () => {
		expect(
			getGroupContentString([
				tab({
					title: 'A'.repeat(40),
				}),
				tab(),
			]),
		).toBe(`${'A'.repeat(29)}… and 1 other tab`);
	});
	it('builds the group hover-card name from at most five titles', () => {
		const tabs = Array.from(
			{
				length: 7,
			},
			(_, index) =>
				tab({
					id: `${index}`,
					title: `Tab ${index + 1}`,
				}),
		);
		const name = getGroupAccessibleName(group, tabs);
		expect(name).toContain('Shared, Work, 7 tabs');
		expect(name).toContain('•  Tab 5');
		expect(name).not.toContain('•  Tab 6');
		expect(name).toContain('+ 2 more, expanded');
		expect(getGroupAccessibleName(group, tabs.slice(0, 6))).toContain('+ 1 more, expanded');
	});
	it("keeps each bulleted accessible group title within Chromium's 50-character cap", () => {
		const name = getGroupAccessibleName(group, [
			tab({
				title: 'A'.repeat(100),
			}),
		]);
		const item = requireDefined(name.split(', ').find((part) => part.startsWith('•')));
		expect(Array.from(item)).toHaveLength(50);
		expect(item.endsWith('…')).toBe(true);
	});
	it('builds named and unnamed fallback group tooltips', () => {
		expect(
			getGroupTooltipText(group, [
				tab({
					title: 'One',
				}),
				tab({
					title: 'Two',
				}),
			]),
		).toBe('Work - One and 1 other tab');
		expect(
			getGroupTooltipText(
				{
					title: '',
				},
				[
					tab({
						title: 'Only',
					}),
				],
			),
		).toBe('Only');
	});
	it('announces shared-group activity only while its indicator is visible', () => {
		const tabs = [
			tab({
				title: 'One',
			}),
		];
		expect(
			getGroupAccessibleName(
				{
					...group,
					attention: true,
					collapsed: false,
				},
				tabs,
			),
		).not.toContain('new activity');
		expect(
			getGroupAccessibleName(
				{
					...group,
					attention: true,
					collapsed: true,
				},
				tabs,
			),
		).toContain('new activity');
	});
	it('wraps tab state in the same priority order as GetAccessibleTabLabel', () => {
		expect(
			getAccessibleTabLabel(
				tab({
					group: 'work',
					pinned: true,
					crashed: true,
				}),
				{
					group,
				},
			),
		).toBe('Document, in group Work, pinned, crashed');
		expect(
			getAccessibleTabLabel(
				tab({
					alertState: 'audio-playing',
					memoryUsageBytes: 125 * 1024 * 1024,
				}),
			),
		).toBe('Document, playing audio, memory usage 125 MB');
		expect(
			getAccessibleTabLabel(
				tab({
					alertState: 'desktop-capturing',
				}),
			),
		).toBe('Document, sharing your screen');
		expect(
			getAccessibleTabLabel(
				tab({
					alertState: 'glic-sharing',
				}),
			),
		).toBe('Document, shared with Gemini');
		expect(
			getAccessibleTabLabel(
				tab({
					discarded: true,
					discardedMemorySavingsBytes: 2 * 1024 * 1024,
				}),
			),
		).toBe('Document, inactive tab, 2 MB memory saved');
	});
	it('describes split positions and collaboration activity', () => {
		expect(
			getAccessibleTabLabel(
				tab({
					split: 'pair',
				}),
				{
					splitIndex: 1,
					splitLayout: 'stacked',
				},
			),
		).toBe('Document, bottom view');
		expect(
			getAccessibleTabLabel(
				tab({
					collaborationMessage: {
						givenName: 'Ari',
						event: 'tab-added',
					},
				}),
			),
		).toBe('Ari added Document');
	});
	it('ports directional and group-membership announcements', () => {
		expect(getTabMoveAnnouncement('previous', 'vertical')).toBe('Tab moved up');
		expect(getTabMoveAnnouncement('next', 'horizontal')).toBe('Tab moved right');
		expect(
			getTabGroupMembershipAnnouncement('added', group, [
				tab({
					title: 'Before',
				}),
			]),
		).toBe('Tab added to group Work, Before');
	});
});
