import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { VerticalTabStripTopContainer } from '../VerticalTabStripTopContainer';

afterEach(cleanup);

describe('VerticalTabStripTopContainer', () => {
	it('lays out expanded controls horizontally and invokes host actions', () => {
		const collapse = vi.fn<() => void>();
		const { container } = render(
			<VerticalTabStripTopContainer
				availableWidth={240}
				collapsed={false}
				showTabSearchButton
				onCollapsedChange={collapse}
			/>,
		);

		expect(
			container
				.querySelector('[data-vertical-top-container]')
				?.getAttribute('data-combo-orientation'),
		).toBe('horizontal');
		fireEvent.click(screen.getByRole('button', { name: 'Collapse tab strip' }));
		expect(collapse).toHaveBeenCalledWith(true);
	});

	it('stacks the combo vertically while collapsed and exposes the focus action', () => {
		const unfocus = vi.fn<() => void>();
		const { container } = render(
			<VerticalTabStripTopContainer
				availableWidth={72}
				collapsed
				showUnfocusButton
				showTabSearchButton
				onUnfocusGroup={unfocus}
			/>,
		);

		expect(
			container
				.querySelector('[data-vertical-top-container]')
				?.getAttribute('data-combo-orientation'),
		).toBe('vertical');
		expect(screen.getByRole('button', { name: 'Search tabs' }).getAttribute('data-flat-edge')).toBe(
			'none',
		);
		expect(container.querySelector('[data-flat-edge-label]')).toBeNull();
		fireEvent.click(screen.getByRole('button', { name: 'Show all tabs' }));
		expect(unfocus).toHaveBeenCalledOnce();
	});
});
