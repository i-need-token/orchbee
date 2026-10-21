// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { VerticalTabStripCollapseState } from '../../../../../../../types/tab-strip-types';
import { VerticalTabStripBottomContainer } from '../VerticalTabStripBottomContainer';

afterEach(cleanup);

describe('VerticalTabStripBottomContainer', () => {
	it('exposes host creation availability on the native button', () => {
		render(
			<VerticalTabStripBottomContainer
				collapseState={VerticalTabStripCollapseState.Expanded}
				disabled
				onNewTab={() => undefined}
			/>,
		);

		expect(screen.getByRole('button', { name: 'New tab' }).hasAttribute('disabled')).toBe(true);
	});

	it('forwards primary and context actions with collapsed styling', () => {
		const onNewTab = vi.fn<() => void>();
		const onContextMenu = vi.fn<(event: React.MouseEvent) => void>((event) =>
			event.preventDefault(),
		);
		const { container } = render(
			<VerticalTabStripBottomContainer
				collapseState={VerticalTabStripCollapseState.Collapsed}
				onNewTab={onNewTab}
				onContextMenu={onContextMenu}
			/>,
		);
		const button = screen.getByRole('button', { name: 'New tab' });
		fireEvent.click(button);
		fireEvent.contextMenu(button);
		expect(onNewTab).toHaveBeenCalledOnce();
		expect(onContextMenu).toHaveBeenCalledOnce();
		expect(container.firstElementChild?.getAttribute('data-orientation')).toBe('vertical');
		expect(button.querySelector('[data-icon="add"]')).not.toBeNull();
		expect(button.querySelector('[data-tab-strip-icon="plus"]')).toBeNull();
		expect(button.querySelector('[data-flat-edge-label]')).toBeNull();
	});
});
