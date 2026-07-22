import { describe, expect, it } from 'vitest';
import {
	formatHoverCardDomain,
	getGroupHoverCardData,
	getHoverCardFooterRows,
	getHoverCardTitleData,
} from '../hover-card-data';

describe('hover-card anchor data', () => {
	it('formats web origins and special URL sources like Chromium', () => {
		expect(formatHoverCardDomain('https://www.chromium.org/developers/design-documents/')).toBe(
			'chromium.org',
		);
		expect(formatHoverCardDomain('file:///Users/test/report.pdf')).toBe('Local file');
		expect(formatHoverCardDomain('blob:https://example.com/id')).toBe('Blob');
		expect(formatHoverCardDomain('view-source:https://example.com')).toBe('View source');
		expect(formatHoverCardDomain('about:blank')).toBe('about:blank');
		expect(formatHoverCardDomain('https://example.com/private', false)).toBe('');
	});

	it('middle-elides file and image-dimension titles', () => {
		expect(
			getHoverCardTitleData('report-final.pdf', 'file:///tmp/report-final.pdf').isFilename,
		).toBe(true);
		expect(
			getHoverCardTitleData('photo.png (640×480)', 'https://example.com/photo.png').isFilename,
		).toBe(true);
		expect(getHoverCardTitleData('Regular page', 'https://example.com').isFilename).toBe(false);
	});

	it('uses discard state instead of alert and suppresses performance behind discard/collaboration', () => {
		expect(
			getHoverCardFooterRows({
				id: 'tab',
				title: 'Tab',
				pinned: false,
				group: null,
				split: null,
				discarded: true,
				discardedMemorySavingsBytes: 2 * 1024 * 1024,
				alertState: 'audio-playing',
				memoryUsageBytes: 125 * 1024 * 1024,
				memoryUsageHigh: true,
				collaborationMessage: { givenName: 'Ari', event: 'tab-updated', avatarUrl: '/ari.png' },
			}),
		).toEqual([
			{ kind: 'discard', text: 'Inactive tab, 2 MB memory saved' },
			{ kind: 'collaboration', text: 'Ari changed this tab', avatarUrl: '/ari.png' },
		]);
	});

	it('honors the memory preference but always surfaces high memory usage', () => {
		const tab = {
			id: 'tab',
			title: 'Tab',
			pinned: false,
			group: null,
			split: null,
			memoryUsageBytes: 1_000,
		} as const;
		expect(getHoverCardFooterRows(tab, { showMemoryUsage: false })).toEqual([]);
		expect(
			getHoverCardFooterRows({ ...tab, memoryUsageHigh: true }, { showMemoryUsage: false }),
		).toEqual([{ kind: 'performance', text: 'High memory usage: 1000 bytes', high: true }]);
	});

	it('maps an ordinary alert to its icon color row', () => {
		expect(
			getHoverCardFooterRows({
				id: 'tab',
				title: 'Tab',
				pinned: false,
				group: null,
				split: null,
				alertState: 'video-recording',
			}),
		).toEqual([
			{
				kind: 'alert',
				text: 'This tab is using your camera',
				alertState: 'video-recording',
				colorRole: 'recording',
			},
		]);
	});

	it("builds named and unnamed group cards with Chromium's five-tab cap", () => {
		const tabs = Array.from(
			{ length: 7 },
			(_, index) =>
				({
					id: `tab-${index}`,
					title: index === 0 ? 'photo.png (640×480)' : `Tab ${index + 1}`,
					url: 'https://example.test',
					pinned: false,
					group: 'group',
					split: null,
				}) as const,
		);
		const data = getGroupHoverCardData(
			{
				id: 'group',
				title: 'Research',
				color: 'blue',
				collapsed: false,
			},
			tabs,
		);
		expect(data.title).toEqual({ text: 'Research · 7 tabs', isFilename: false });
		expect(data.tabTitles).toHaveLength(5);
		expect(data.tabTitles[0]).toEqual({ text: 'photo.png (640×480)', isFilename: true });
		expect(data.excessText).toBe('+ 2 more');

		expect(
			getGroupHoverCardData(
				{
					id: 'group',
					title: '',
					color: 'blue',
					collapsed: false,
				},
				tabs.slice(0, 1),
			).title.text,
		).toBe('1 tab in unnamed group');
	});
});
