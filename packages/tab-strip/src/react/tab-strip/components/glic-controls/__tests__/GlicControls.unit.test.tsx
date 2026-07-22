import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { GlicButtonWidthState, GlicControlsController } from '../../../../../glic';
import { GlicControls } from '../GlicControls';

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

describe('GlicControls', () => {
	it('toggles the Gemini highlight and actor pressed state', () => {
		const controller = new GlicControlsController({ actorVisible: true });
		render(<GlicControls controller={controller} />);
		const controls = screen.getByRole('group', { name: 'Gemini and actor controls' });

		fireEvent.click(screen.getByRole('button', { name: 'Open Gemini' }));
		expect(controller.getSnapshot().panelOpen).toBe(true);
		expect(controls.hasAttribute('data-highlighted')).toBe(true);
		expect(screen.getByRole('button', { name: 'Close Gemini' })).toBeTruthy();

		fireEvent.click(screen.getByRole('button', { name: 'Actor tasks' }));
		expect(controller.getSnapshot().actorPressed).toBe(true);
	});

	it('renders and dismisses a Chromium nudge', () => {
		vi.useFakeTimers();
		const controller = new GlicControlsController();
		controller.triggerGlicNudge('Summarize this page');
		render(<GlicControls controller={controller} />);

		expect(screen.getByText('Summarize this page')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Dismiss Gemini suggestion' }));
		expect(controller.getSnapshot().widthState).toBe(GlicButtonWidthState.Nudge);
		expect(screen.getByRole('button', { name: 'Dismiss Gemini suggestion' })).toBeTruthy();
		act(() => vi.advanceTimersByTime(250));
		expect(controller.getSnapshot().widthState).toBe(GlicButtonWidthState.Normal);
		expect(screen.queryByRole('button', { name: 'Dismiss Gemini suggestion' })).toBeNull();
	});

	it('exposes the unpin command only while pinned', () => {
		const controller = new GlicControlsController();
		const onUnpin = vi.fn<() => void>();
		render(<GlicControls controller={controller} onUnpin={onUnpin} />);
		const button = screen.getByRole('button', { name: 'Open Gemini' });
		fireEvent.contextMenu(button, { clientX: 20, clientY: 20 });
		fireEvent.click(screen.getByRole('menuitem', { name: 'Unpin Gemini' }));
		expect(controller.getSnapshot().pinned).toBe(false);
		expect(onUnpin).toHaveBeenCalledOnce();
		fireEvent.contextMenu(button, { clientX: 20, clientY: 20 });
		expect(screen.queryByRole('menu')).toBeNull();
	});

	it('holds a requested expansion under the pointer and flushes it on mouse leave', () => {
		const controller = new GlicControlsController({ richAnimations: false });
		render(<GlicControls controller={controller} />);
		const controls = screen.getByRole('group', { name: 'Gemini and actor controls' });
		fireEvent.mouseEnter(controls);
		controller.triggerGlicNudge('Explain this page');
		expect(screen.queryByText('Explain this page')).toBeNull();
		fireEvent.mouseLeave(controls);
		expect(screen.getByText('Explain this page')).toBeTruthy();
	});

	it('resets the Gemini nudge when the active tab changes', () => {
		const controller = new GlicControlsController({ richAnimations: false });
		controller.triggerGlicNudge('Summarize');
		const view = render(<GlicControls controller={controller} activeTabId='a' />);
		expect(screen.getByText('Summarize')).toBeTruthy();
		view.rerender(<GlicControls controller={controller} activeTabId='b' />);
		expect(screen.queryByText('Summarize')).toBeNull();
		expect(controller.getSnapshot().nudgeLabel).toBe('');
	});
});
