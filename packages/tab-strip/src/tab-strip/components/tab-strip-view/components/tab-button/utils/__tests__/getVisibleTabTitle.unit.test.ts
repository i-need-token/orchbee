import { describe, expect, it } from 'vitest';
import { getVisibleTabTitle } from '../getVisibleTabTitle';

const base = {
	id: 'tab',
	title: 'Document',
	pinned: false,
	group: null,
} as const;

describe('TabTitle', () => {
	it('uses the normal page title by default', () => {
		expect(getVisibleTabTitle(base)).toBe('Document');
		expect(getVisibleTabTitle({ ...base, title: '' })).toBe('New Tab');
	});

	it('tracks the actual throbber when a loading title is requested', () => {
		expect(
			getVisibleTabTitle({
				...base,
				shouldRenderLoadingTitle: true,
				networkState: 'loading',
			}),
		).toBe('Loading…');
		expect(
			getVisibleTabTitle({
				...base,
				shouldRenderLoadingTitle: true,
				networkState: 'loading',
				shouldHideThrobber: true,
			}),
		).toBe('New Tab');
		expect(
			getVisibleTabTitle({
				...base,
				shouldRenderLoadingTitle: true,
				networkState: 'error',
			}),
		).toBe('New Tab');
	});
});
