import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { VerticalTabStripTopContainer } from '../VerticalTabStripTopContainer';

afterEach(cleanup);

describe('VerticalTabStripTopContainer', () => {
	it('lays out expanded controls horizontally and invokes host actions', () => {
		const collapse = vi.fn<() => void>();
		const projects = vi.fn<() => void>();
		const { container } = render(
			<VerticalTabStripTopContainer
				availableWidth={240}
				collapsed={false}
				showProjectsButton
				showTabSearchButton
				onCollapsedChange={collapse}
				onProjectsClick={projects}
			/>,
		);

		expect(
			container.querySelector('.ct-vertical-top-container')?.getAttribute('data-combo-orientation'),
		).toBe('horizontal');
		fireEvent.click(screen.getByRole('button', { name: 'Collapse tab strip' }));
		fireEvent.click(screen.getByRole('button', { name: 'Projects' }));
		expect(collapse).toHaveBeenCalledWith(true);
		expect(projects).toHaveBeenCalledOnce();
	});

	it('stacks the combo vertically while collapsed and exposes the focus action', () => {
		const unfocus = vi.fn<() => void>();
		const { container } = render(
			<VerticalTabStripTopContainer
				availableWidth={72}
				collapsed
				showUnfocusButton
				showProjectsButton
				showTabSearchButton
				onUnfocusGroup={unfocus}
			/>,
		);

		expect(
			container.querySelector('.ct-vertical-top-container')?.getAttribute('data-combo-orientation'),
		).toBe('vertical');
		expect(screen.getByRole('button', { name: 'Projects' }).getAttribute('data-flat-edge')).toBe(
			'bottom',
		);
		expect(screen.getByRole('button', { name: 'Search tabs' }).getAttribute('data-flat-edge')).toBe(
			'top',
		);
		expect(container.querySelector('.ct-flat-edge-button__label')).toBeNull();
		fireEvent.click(screen.getByRole('button', { name: 'Show all tabs' }));
		expect(unfocus).toHaveBeenCalledOnce();
	});
});
