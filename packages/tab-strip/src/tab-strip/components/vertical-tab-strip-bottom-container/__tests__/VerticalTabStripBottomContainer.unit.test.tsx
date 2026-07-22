import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { VerticalTabStripCollapseState } from '../../../../common';
import { VerticalTabStripBottomContainer } from '../VerticalTabStripBottomContainer';

afterEach(cleanup);

describe('VerticalTabStripBottomContainer', () => {
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
		expect(button.querySelector('[data-flat-edge-label]')).toBeNull();
	});
});
