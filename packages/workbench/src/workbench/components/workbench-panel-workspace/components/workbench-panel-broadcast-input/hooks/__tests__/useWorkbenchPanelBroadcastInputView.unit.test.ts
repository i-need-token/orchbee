// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchTerminalBroadcastInput } from '../../../../../../types/workbench-terminal-surface';
import { useWorkbenchPanelBroadcastInputView } from '../useWorkbenchPanelBroadcastInputView';

interface HarnessProps {
	readonly end: () => void;
	readonly enqueue: (input: WorkbenchTerminalBroadcastInput) => void;
	readonly toggleVisibility: () => void;
}

function Harness(properties: HarnessProps): ReactNode {
	const view = useWorkbenchPanelBroadcastInputView(properties);
	return createElement(
		'div',
		null,
		createElement('input', {
			'aria-label': 'Broadcast input',
			onBeforeInput: view.onBeforeInput,
			onBlur: view.onBlur,
			onCompositionEnd: view.onCompositionEnd,
			onCompositionStart: view.onCompositionStart,
			onDrop: view.onDrop,
			onInput: view.onInput,
			onKeyDown: view.onKeyDown,
			onKeyUp: view.onKeyUp,
			onPaste: view.onPaste,
			ref: view.inputReference,
		}),
		createElement('button', { onClick: view.toggleVisibility, type: 'button' }, 'Hide'),
		createElement('button', { onClick: view.end, type: 'button' }, 'End'),
	);
}

afterEach(() => cleanup());

describe('useWorkbenchPanelBroadcastInputView', () => {
	it('admits one committed composition and physical keys while excluding paste and control chords', async () => {
		const end = vi.fn<() => void>();
		const enqueue = vi.fn<(input: WorkbenchTerminalBroadcastInput) => void>();
		const toggleVisibility = vi.fn<() => void>();
		render(createElement(Harness, { end, enqueue, toggleVisibility }));
		const input = screen.getByRole<HTMLInputElement>('textbox', { name: 'Broadcast input' });

		fireEvent.input(input, {
			data: 'forbidden',
			inputType: 'insertFromPaste',
			target: { value: 'forbidden' },
		});
		expect(enqueue).not.toHaveBeenCalled();
		expect(input.value).toBe('');

		fireEvent.compositionStart(input);
		fireEvent.input(input, {
			data: '部署',
			inputType: 'insertCompositionText',
			isComposing: true,
			target: { value: '部署' },
		});
		fireEvent.compositionEnd(input, { data: '部署' });
		fireEvent.input(input, {
			data: '部署',
			inputType: 'insertText',
			target: { value: '部署' },
		});
		await act(async () => await Promise.resolve());
		expect(enqueue).toHaveBeenCalledExactlyOnceWith({ kind: 'text', text: '部署' });

		fireEvent.keyDown(input, { code: 'Enter', key: 'Enter' });
		fireEvent.keyDown(input, { code: 'Escape', key: 'Escape' });
		expect(enqueue).toHaveBeenCalledTimes(3);
		expect(enqueue).toHaveBeenNthCalledWith(
			2,
			expect.objectContaining({ code: 'Enter', key: 'Enter', kind: 'key' }),
		);
		expect(enqueue).toHaveBeenNthCalledWith(
			3,
			expect.objectContaining({ code: 'Escape', key: 'Escape', kind: 'key' }),
		);

		fireEvent.keyDown(input, { code: 'KeyV', ctrlKey: true, key: 'v' });
		fireEvent.keyDown(input, { code: 'KeyA', key: 'a', metaKey: true });
		fireEvent.keyDown(input, { altKey: true, code: 'Escape', ctrlKey: true, key: 'Escape' });
		fireEvent.keyDown(input, { code: 'Escape', ctrlKey: true, key: 'Escape' });
		expect(enqueue).toHaveBeenCalledTimes(3);
		expect(toggleVisibility).toHaveBeenCalledOnce();
		expect(end).toHaveBeenCalledOnce();

		const hide = screen.getByRole('button', { name: 'Hide' });
		hide.focus();
		fireEvent.click(hide);
		await act(async () => await Promise.resolve());
		expect(toggleVisibility).toHaveBeenCalledTimes(2);
		expect(input.ownerDocument.activeElement).toBe(input);
	});
});
