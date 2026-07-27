// @vitest-environment jsdom

import type { WorkbenchActionId } from '../../actions/workbench-action-ids';
import { cleanup, render } from '@testing-library/react';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { useWorkbenchShortcuts } from '../useWorkbenchShortcuts';

interface WorkbenchShortcutsHarnessProps {
	readonly invokeIfEnabled: (actionId: WorkbenchActionId, source: 'keyboard') => boolean;
}

function WorkbenchShortcutsHarness({ invokeIfEnabled }: WorkbenchShortcutsHarnessProps) {
	useWorkbenchShortcuts('apple', invokeIfEnabled);
	return null;
}

afterEach(() => cleanup());

describe('useWorkbenchShortcuts', () => {
	it('can mount during server rendering without invoking an action', () => {
		const invokeIfEnabled = vi.fn<(actionId: WorkbenchActionId, source: 'keyboard') => boolean>(
			() => true,
		);

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
		const invokeIfEnabled = vi.fn<(actionId: WorkbenchActionId, source: 'keyboard') => boolean>(
			() => true,
		);
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

		expect(invokeIfEnabled).toHaveBeenCalledWith('workbench.terminal.create', 'keyboard');
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
