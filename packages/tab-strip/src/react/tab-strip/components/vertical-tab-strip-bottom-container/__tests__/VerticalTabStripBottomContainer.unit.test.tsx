import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { VerticalTabStripCollapseState } from '../../../../../common';
import { VerticalTabStripBottomContainer } from '../VerticalTabStripBottomContainer';

afterEach(cleanup);

describe('VerticalTabStripBottomContainer', () => {
	it('forwards primary, middle, and context actions with collapsed styling', () => {
		const onNewTab = vi.fn<() => void>();
		const onClipboard = vi.fn<() => void>();
		const onContextMenu = vi.fn<(event: React.MouseEvent) => void>((event: React.MouseEvent) =>
			event.preventDefault(),
		);
		const { container } = render(
			<VerticalTabStripBottomContainer
				collapseState={VerticalTabStripCollapseState.Collapsed}
				onNewTab={onNewTab}
				onNewTabFromClipboard={onClipboard}
				onContextMenu={onContextMenu}
			/>,
		);
		const button = screen.getByRole('button', { name: 'New tab' });
		fireEvent.click(button);
		fireEvent.pointerDown(button, { button: 1, pointerType: 'mouse' });
		fireEvent(button, new MouseEvent('auxclick', { button: 1, bubbles: true }));
		fireEvent.contextMenu(button);
		expect(onNewTab).toHaveBeenCalledOnce();
		expect(onClipboard).toHaveBeenCalledOnce();
		expect(onContextMenu).toHaveBeenCalledOnce();
		expect(container.firstElementChild?.getAttribute('data-orientation')).toBe('vertical');
		expect(button.querySelector('.ct-flat-edge-button__label')).toBeNull();
	});
});
