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
	readonly invokeIfEnabled: (binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean;
}

function WorkbenchShortcutsHarness({ invokeIfEnabled }: WorkbenchShortcutsHarnessProps) {
	const onKeyDownCapture = useWorkbenchShortcuts(
		'apple',
		defaultWorkbenchShortcutBindings('apple'),
		invokeIfEnabled,
	);
	return createElement(
		'div',
		{ onKeyDownCapture },
		createElement('textarea', { 'aria-label': 'Terminal input' }),
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
});
