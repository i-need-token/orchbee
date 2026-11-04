// @vitest-environment jsdom

import { cleanup, render, screen } from '@testing-library/react';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { useWorkbenchShortcuts } from '../useWorkbenchShortcuts';
import {
	defaultWorkbenchShortcutBindings,
	type WorkbenchShortcutBinding,
} from '../../utils/workbench-shortcuts';

interface WorkbenchShortcutsHarnessProps {
	readonly active?: boolean | undefined;
	readonly invokeIfEnabled: (binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean;
}

function WorkbenchShortcutsHarness({
	active = true,
	invokeIfEnabled,
}: WorkbenchShortcutsHarnessProps) {
	const rootReference = useWorkbenchShortcuts(
		active,
		'apple',
		defaultWorkbenchShortcutBindings('apple'),
		invokeIfEnabled,
	);
	return createElement(
		'div',
		{ ref: rootReference },
		createElement('textarea', { 'aria-label': 'Terminal input' }),
		createElement(
			'div',
			{ 'data-workbench-keyboard-capture': 'true' },
			createElement('button', { 'aria-label': 'Exclusive keyboard capture' }),
		),
		createElement(
			'div',
			{ role: 'dialog' },
			createElement('button', { 'aria-label': 'Dialog action' }),
		),
	);
}

afterEach(() => cleanup());

describe('useWorkbenchShortcuts', () => {
	it('can mount during server rendering without invoking an action', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);

		renderToStaticMarkup(
			createElement(WorkbenchShortcutsHarness, {
				invokeIfEnabled,
			}),
		);
		expect(invokeIfEnabled).not.toHaveBeenCalled();
	});

	it('captures an accepted global shortcut before focused terminal input', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		render(
			createElement(WorkbenchShortcutsHarness, {
				invokeIfEnabled,
			}),
		);
		const input = screen.getByRole('textbox', { name: 'Terminal input' });
		const terminalKeyDown = vi.fn<(event: Event) => void>();
		input.addEventListener('keydown', terminalKeyDown);
		const accepted = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'KeyB',
			metaKey: true,
		});

		input.dispatchEvent(accepted);

		expect(invokeIfEnabled).toHaveBeenCalledWith(
			expect.objectContaining({ shortcutId: 'workbench.sidebar.toggle' }),
			'keyboard',
		);
		expect(accepted.defaultPrevented).toBe(true);
		expect(terminalKeyDown).not.toHaveBeenCalled();
	});

	it('leaves events inside an exclusive keyboard scope to that scope', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		render(createElement(WorkbenchShortcutsHarness, { invokeIfEnabled }));
		const capture = screen.getByRole('button', { name: 'Exclusive keyboard capture' });
		const event = new KeyboardEvent('keydown', {
			bubbles: true,
			cancelable: true,
			code: 'KeyT',
			metaKey: true,
		});

		capture.dispatchEvent(event);

		expect(invokeIfEnabled).not.toHaveBeenCalled();
		expect(event.defaultPrevented).toBe(false);
	});

	it('leaves events inside a modal dialog to that dialog', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		render(createElement(WorkbenchShortcutsHarness, { invokeIfEnabled }));
		const dialogAction = screen.getByRole('button', { name: 'Dialog action' });
		const event = new KeyboardEvent('keydown', {
			bubbles: true,
			cancelable: true,
			code: 'KeyT',
			metaKey: true,
		});

		dialogAction.dispatchEvent(event);

		expect(invokeIfEnabled).not.toHaveBeenCalled();
		expect(event.defaultPrevented).toBe(false);
	});

	it('captures an active Workbench shortcut when no focused descendant survives', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		render(createElement(WorkbenchShortcutsHarness, { invokeIfEnabled }));
		const accepted = new KeyboardEvent('keydown', {
			bubbles: true,
			cancelable: true,
			code: 'KeyT',
			metaKey: true,
		});

		document.body.dispatchEvent(accepted);

		expect(invokeIfEnabled).toHaveBeenCalledWith(
			expect.objectContaining({ shortcutId: 'workbench.terminal.create' }),
			'keyboard',
		);
		expect(accepted.defaultPrevented).toBe(true);
	});

	it('captures positional Tab selection before focused terminal input', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		render(createElement(WorkbenchShortcutsHarness, { invokeIfEnabled }));
		const input = screen.getByRole('textbox', { name: 'Terminal input' });
		const accepted = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'Digit2',
			metaKey: true,
		});

		input.dispatchEvent(accepted);

		expect(invokeIfEnabled).toHaveBeenCalledWith(
			expect.objectContaining({
				action: 'selectTab2',
				shortcutId: 'workbench.tab.select.2',
			}),
			'keyboard',
		);
		expect(accepted.defaultPrevented).toBe(true);
	});

	it('captures exact positional Panel focus and preserves unavailable positions for terminal input', () => {
		const available = vi.fn<(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean>(
			() => true,
		);
		const rendered = render(
			createElement(WorkbenchShortcutsHarness, {
				invokeIfEnabled: available,
			}),
		);
		const input = screen.getByRole('textbox', { name: 'Terminal input' });
		const accepted = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'Digit0',
			ctrlKey: true,
			shiftKey: true,
		});

		input.dispatchEvent(accepted);

		expect(available).toHaveBeenCalledWith(
			expect.objectContaining({
				action: 'focusPanel10',
				shortcutId: 'workbench.panel.focus.10',
			}),
			'keyboard',
		);
		expect(accepted.defaultPrevented).toBe(true);

		rendered.unmount();
		const unavailable = vi.fn<(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean>(
			() => false,
		);
		render(createElement(WorkbenchShortcutsHarness, { invokeIfEnabled: unavailable }));
		const currentInput = screen.getByRole('textbox', { name: 'Terminal input' });
		const terminalKeyDown = vi.fn<(event: Event) => void>();
		currentInput.addEventListener('keydown', terminalKeyDown);
		const preserved = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'Digit9',
			ctrlKey: true,
			shiftKey: true,
		});

		currentInput.dispatchEvent(preserved);

		expect(unavailable).toHaveBeenCalledWith(
			expect.objectContaining({
				action: 'focusPanel9',
				shortcutId: 'workbench.panel.focus.9',
			}),
			'keyboard',
		);
		expect(preserved.defaultPrevented).toBe(false);
		expect(terminalKeyDown).toHaveBeenCalledOnce();
	});

	it('captures adjacent Tab selection before focused terminal input', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		render(createElement(WorkbenchShortcutsHarness, { invokeIfEnabled }));
		const input = screen.getByRole('textbox', { name: 'Terminal input' });
		const accepted = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'BracketRight',
			metaKey: true,
			shiftKey: true,
		});

		input.dispatchEvent(accepted);

		expect(invokeIfEnabled).toHaveBeenCalledWith(
			expect.objectContaining({
				action: 'selectNextTab',
				shortcutId: 'workbench.tab.select-next',
			}),
			'keyboard',
		);
		expect(accepted.defaultPrevented).toBe(true);
	});

	it('captures cyclic Panel focus and preserves unavailable chords for terminal input', () => {
		const available = vi.fn<(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean>(
			() => true,
		);
		const rendered = render(
			createElement(WorkbenchShortcutsHarness, {
				invokeIfEnabled: available,
			}),
		);
		const input = screen.getByRole('textbox', { name: 'Terminal input' });
		const accepted = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'BracketRight',
			ctrlKey: true,
			shiftKey: true,
		});

		input.dispatchEvent(accepted);

		expect(available).toHaveBeenCalledWith(
			expect.objectContaining({
				action: 'focusActivePanelNext',
				shortcutId: 'workbench.panel.active.focus-next',
			}),
			'keyboard',
		);
		expect(accepted.defaultPrevented).toBe(true);

		rendered.unmount();
		const unavailable = vi.fn<(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean>(
			() => false,
		);
		render(
			createElement(WorkbenchShortcutsHarness, {
				invokeIfEnabled: unavailable,
			}),
		);
		const currentInput = screen.getByRole('textbox', { name: 'Terminal input' });
		const terminalKeyDown = vi.fn<(event: Event) => void>();
		currentInput.addEventListener('keydown', terminalKeyDown);
		const preserved = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'BracketLeft',
			ctrlKey: true,
			shiftKey: true,
		});

		currentInput.dispatchEvent(preserved);

		expect(unavailable).toHaveBeenCalledWith(
			expect.objectContaining({
				action: 'focusActivePanelPrevious',
				shortcutId: 'workbench.panel.active.focus-previous',
			}),
			'keyboard',
		);
		expect(preserved.defaultPrevented).toBe(false);
		expect(terminalKeyDown).toHaveBeenCalledOnce();
	});

	it('captures active Tab movement before focused terminal input', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		render(createElement(WorkbenchShortcutsHarness, { invokeIfEnabled }));
		const input = screen.getByRole('textbox', { name: 'Terminal input' });
		const accepted = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'Period',
			ctrlKey: true,
			shiftKey: true,
		});

		input.dispatchEvent(accepted);

		expect(invokeIfEnabled).toHaveBeenCalledWith(
			expect.objectContaining({
				action: 'moveActiveTabForward',
				shortcutId: 'workbench.tab.active.move-forward',
			}),
			'keyboard',
		);
		expect(accepted.defaultPrevented).toBe(true);
	});

	it('captures Tab title editing before focused terminal input', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		render(createElement(WorkbenchShortcutsHarness, { invokeIfEnabled }));
		const input = screen.getByRole('textbox', { name: 'Terminal input' });
		const accepted = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'KeyI',
			metaKey: true,
			shiftKey: true,
		});

		input.dispatchEvent(accepted);

		expect(invokeIfEnabled).toHaveBeenCalledWith(
			expect.objectContaining({
				action: 'setTabTitle',
				shortcutId: 'workbench.tab.title.set',
			}),
			'keyboard',
		);
		expect(accepted.defaultPrevented).toBe(true);
	});

	it('does not participate while its semantic scope is inactive', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		render(
			createElement(WorkbenchShortcutsHarness, {
				active: false,
				invokeIfEnabled,
			}),
		);
		const skipped = new KeyboardEvent('keydown', {
			bubbles: true,
			cancelable: true,
			code: 'KeyT',
			metaKey: true,
		});

		document.body.dispatchEvent(skipped);

		expect(invokeIfEnabled).not.toHaveBeenCalled();
		expect(skipped.defaultPrevented).toBe(false);
	});

	it('leaves terminal input untouched when the Action is unavailable', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => false);
		render(
			createElement(WorkbenchShortcutsHarness, {
				invokeIfEnabled,
			}),
		);
		const input = screen.getByRole('textbox', { name: 'Terminal input' });
		const terminalKeyDown = vi.fn<(event: Event) => void>();
		input.addEventListener('keydown', terminalKeyDown);

		const unavailable = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'KeyB',
			metaKey: true,
		});

		input.dispatchEvent(unavailable);

		expect(unavailable.defaultPrevented).toBe(false);
		expect(terminalKeyDown).toHaveBeenCalledOnce();
	});

	it('removes its document listener when Workbench unmounts', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		const rendered = render(createElement(WorkbenchShortcutsHarness, { invokeIfEnabled }));
		rendered.unmount();

		document.body.dispatchEvent(
			new KeyboardEvent('keydown', {
				bubbles: true,
				cancelable: true,
				code: 'KeyT',
				metaKey: true,
			}),
		);

		expect(invokeIfEnabled).not.toHaveBeenCalled();
	});
});
