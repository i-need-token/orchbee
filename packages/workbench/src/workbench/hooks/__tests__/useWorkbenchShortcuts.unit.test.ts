// @vitest-environment jsdom

import { cleanup, render } from '@testing-library/react';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { useWorkbenchShortcuts } from '../useWorkbenchShortcuts';
import type { WorkbenchShortcutBinding } from '../../utils/workbench-shortcuts';

interface WorkbenchShortcutsHarnessProps {
	readonly invokeIfEnabled: (binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean;
}

function WorkbenchShortcutsHarness({ invokeIfEnabled }: WorkbenchShortcutsHarnessProps) {
	useWorkbenchShortcuts('apple', invokeIfEnabled);
	return null;
}

afterEach(() => cleanup());

describe('useWorkbenchShortcuts', () => {
	it('can mount during server rendering without invoking an action', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);

		expect(
			renderToStaticMarkup(
				createElement(WorkbenchShortcutsHarness, {
					invokeIfEnabled,
				}),
			),
		).toBe('');
		expect(invokeIfEnabled).not.toHaveBeenCalled();
	});

	it('prevents the browser shortcut only after an enabled Action accepts it', () => {
		const invokeIfEnabled = vi.fn<
			(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean
		>(() => true);
		const rendered = render(
			createElement(WorkbenchShortcutsHarness, {
				invokeIfEnabled,
			}),
		);
		const accepted = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'KeyN',
			metaKey: true,
		});

		window.dispatchEvent(accepted);

		expect(invokeIfEnabled).toHaveBeenCalledWith(
			expect.objectContaining({ id: 'workbench.terminal.create' }),
			'keyboard',
		);
		expect(accepted.defaultPrevented).toBe(true);

		rendered.rerender(
			createElement(WorkbenchShortcutsHarness, {
				invokeIfEnabled: () => false,
			}),
		);
		const unavailable = new KeyboardEvent('keydown', {
			cancelable: true,
			code: 'KeyK',
			metaKey: true,
		});

		window.dispatchEvent(unavailable);

		expect(unavailable.defaultPrevented).toBe(false);
	});
});
