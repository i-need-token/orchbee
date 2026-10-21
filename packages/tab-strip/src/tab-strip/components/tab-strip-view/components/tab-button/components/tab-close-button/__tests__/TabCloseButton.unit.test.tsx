// @vitest-environment jsdom

import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TabCloseButton } from '../TabCloseButton';

describe('TabCloseButton', () => {
	it('closes on a primary click and renders the product close icon', () => {
		const onClose = vi.fn<() => void>();
		render(<TabCloseButton tabTitle='Docs' onClose={onClose} />);
		const close = screen.getByRole('button', { name: 'Close' });
		expect(close.getAttribute('tabindex')).toBe('-1');
		expect(close.querySelector('[data-icon="close"]')).not.toBeNull();
		expect(close.querySelector('[data-tab-strip-icon="close"]')).toBeNull();
		fireEvent.click(close);
		expect(onClose).toHaveBeenCalledOnce();
	});

	it('lets middle-pointer presses continue to the parent tab', () => {
		const parent = vi.fn<() => void>();
		const { container } = render(
			<div onPointerDown={parent}>
				<TabCloseButton tabTitle='Docs' onClose={vi.fn<() => void>()} />
			</div>,
		);
		fireEvent.pointerDown(within(container).getByRole('button'), {
			button: 1,
			pointerType: 'mouse',
			clientX: 14,
			clientY: 14,
		});
		expect(parent).toHaveBeenCalledOnce();
	});

	it('does not take focus on a right-pointer press', () => {
		const parent = vi.fn<() => void>();
		const { container } = render(
			<div onPointerDown={parent}>
				<TabCloseButton tabTitle='Docs' onClose={vi.fn<() => void>()} />
			</div>,
		);
		const button = within(container).getByRole('button');
		const event = new MouseEvent('pointerdown', { button: 2, bubbles: true, cancelable: true });
		button.dispatchEvent(event);
		expect(event.defaultPrevented).toBe(true);
		expect(parent).toHaveBeenCalledOnce();
		expect(document.activeElement).not.toBe(button);
	});
});
