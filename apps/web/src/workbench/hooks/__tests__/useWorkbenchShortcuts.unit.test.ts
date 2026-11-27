// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import type { KeyboardShortcutSequence } from '@orchbee/keyboard-shortcuts';
import { cleanup, render, screen } from '@testing-library/react';
import { createElement, useRef, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	useWorkbenchKeyboardScope,
	WorkbenchKeyboardScopeContext,
	WorkbenchKeyboardScopeRegistry,
	type WorkbenchKeyboardScope,
} from '../../../workbench-keyboard-scopes';
import { useWorkbenchShortcuts, type WorkbenchShortcutScopes } from '../useWorkbenchShortcuts';
import {
	defaultWorkbenchShortcutBindings,
	type WorkbenchScopedShortcutBinding,
	type WorkbenchScopedShortcutScope,
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
		() => false,
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

function scopedBinding(
	actionId: string,
	scope: WorkbenchScopedShortcutScope,
	sequence: KeyboardShortcutSequence,
): WorkbenchScopedShortcutBinding {
	return {
		definition: {
			actionId,
			defaults: { apple: sequence, standard: sequence },
			scope,
			shortcutId: actionId,
		},
		sequence,
	};
}

const CLOSE_TAB = scopedBinding('file-overlay.close', 'file-bar', [
	{ code: 'KeyK', modifiers: ['mod'] },
	{ code: 'KeyW', modifiers: ['mod'] },
]);
const RENAME = scopedBinding('file-explorer.rename', 'widget:explorer', [
	{ code: 'F2', modifiers: [] },
]);
const EXPLORER_ACTION_SCOPE = new ActionRuntime().createScope('explorer');
const FILE_BAR_SCOPE: WorkbenchKeyboardScope = { kind: 'file-bar' };
const EXPLORER_SCOPE: WorkbenchKeyboardScope = {
	actionScope: EXPLORER_ACTION_SCOPE,
	kind: 'widget',
	widget: 'explorer',
};
/** Monaco's Insert Line Above, which the window binds to Maximize Panel. */
const EDITOR_SCOPE: WorkbenchKeyboardScope = {
	kind: 'code-editor',
	yields: [{ code: 'Enter', modifiers: ['mod', 'shift'] }],
};

function ScopedRegion({
	children,
	label,
	scope,
}: {
	readonly children?: ReactNode;
	readonly label: string;
	readonly scope: WorkbenchKeyboardScope;
}) {
	const reference = useRef<HTMLDivElement>(null);
	useWorkbenchKeyboardScope(reference, scope);
	return createElement('div', { 'aria-label': label, ref: reference, role: 'group' }, children);
}

function ScopedShortcutsHarness({
	invokeShortcut,
	scopes,
}: {
	readonly invokeShortcut: (binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean;
	readonly scopes: WorkbenchShortcutScopes;
}) {
	const rootReference = useWorkbenchShortcuts(
		true,
		'apple',
		defaultWorkbenchShortcutBindings('apple'),
		invokeShortcut,
		() => false,
		scopes,
	);
	return createElement(
		WorkbenchKeyboardScopeContext.Provider,
		{ value: scopes.registry },
		createElement(
			'div',
			{ ref: rootReference },
			createElement('textarea', { 'aria-label': 'Terminal input' }),
			createElement(
				ScopedRegion,
				{ label: 'File Bar', scope: FILE_BAR_SCOPE },
				createElement(
					ScopedRegion,
					{ label: 'Explorer', scope: EXPLORER_SCOPE },
					createElement('button', { 'aria-label': 'Explorer row' }),
					createElement('input', { 'aria-label': 'Rename field' }),
				),
				createElement(
					ScopedRegion,
					{ label: 'Editor', scope: EDITOR_SCOPE },
					createElement('textarea', { 'aria-label': 'Editor input' }),
				),
			),
		),
	);
}

function renderScopedShortcuts() {
	const invoke = vi.fn<WorkbenchShortcutScopes['invoke']>(() => true);
	const invokeShortcut = vi.fn<(binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean>(
		() => true,
	);
	const reportSequence = vi.fn<WorkbenchShortcutScopes['reportSequence']>();
	const scopes: WorkbenchShortcutScopes = {
		bindings: [CLOSE_TAB, RENAME],
		invoke,
		registry: new WorkbenchKeyboardScopeRegistry(),
		reportSequence,
	};
	render(createElement(ScopedShortcutsHarness, { invokeShortcut, scopes }));
	return {
		editor: screen.getByRole('textbox', { name: 'Editor input' }),
		invoke,
		invokeShortcut,
		renameField: screen.getByRole('textbox', { name: 'Rename field' }),
		reportSequence,
		row: screen.getByRole('button', { name: 'Explorer row' }),
		terminal: screen.getByRole('textbox', { name: 'Terminal input' }),
	};
}

function press(target: Element, code: string, init: KeyboardEventInit = {}): KeyboardEvent {
	const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, code, ...init });
	target.dispatchEvent(event);
	return event;
}

describe('useWorkbenchShortcuts keyboard scopes', () => {
	afterEach(() => vi.useRealTimers());

	it('runs a File Bar sequence and leaves Mod+K to a focused terminal', () => {
		const { invoke, reportSequence, row, terminal } = renderScopedShortcuts();
		const terminalKeyDown = vi.fn<(event: Event) => void>();
		terminal.addEventListener('keydown', terminalKeyDown);

		expect(press(row, 'KeyK', { metaKey: true }).defaultPrevented).toBe(true);
		expect(reportSequence).toHaveBeenLastCalledWith({ kind: 'waiting', leader: '⌘K' });
		expect(press(row, 'KeyW', { metaKey: true }).defaultPrevented).toBe(true);
		expect(invoke).toHaveBeenCalledWith(CLOSE_TAB.definition, undefined);
		expect(reportSequence).toHaveBeenLastCalledWith(null);

		expect(press(terminal, 'KeyK', { metaKey: true }).defaultPrevented).toBe(false);
		expect(terminalKeyDown).toHaveBeenCalledOnce();
		expect(invoke).toHaveBeenCalledOnce();
	});

	it('reports a missed step for 10 seconds and ends a waiting sequence after 5 seconds, on blur, or in a terminal', () => {
		vi.useFakeTimers();
		const { invoke, reportSequence, row, terminal } = renderScopedShortcuts();
		const missed = { kind: 'missed', leader: '⌘K', step: 'X' } as const;

		press(row, 'KeyK', { metaKey: true });
		expect(press(row, 'KeyX').defaultPrevented).toBe(true);
		expect(reportSequence).toHaveBeenLastCalledWith(missed);
		vi.advanceTimersByTime(9_999);
		expect(reportSequence).toHaveBeenLastCalledWith(missed);
		vi.advanceTimersByTime(1);
		expect(reportSequence).toHaveBeenLastCalledWith(null);

		// Mod+J names nothing: as a second step it is missed, as a first key it goes on.
		press(row, 'KeyK', { metaKey: true });
		vi.advanceTimersByTime(5_000);
		expect(reportSequence).toHaveBeenLastCalledWith(null);
		expect(press(row, 'KeyJ', { metaKey: true }).defaultPrevented).toBe(false);

		press(row, 'KeyK', { metaKey: true });
		window.dispatchEvent(new Event('blur'));
		expect(reportSequence).toHaveBeenLastCalledWith(null);
		expect(press(row, 'KeyJ', { metaKey: true }).defaultPrevented).toBe(false);

		press(row, 'KeyK', { metaKey: true });
		expect(press(terminal, 'KeyJ', { metaKey: true }).defaultPrevented).toBe(false);
		expect(reportSequence).toHaveBeenLastCalledWith(null);
		expect(invoke).not.toHaveBeenCalled();
	});

	it('runs a widget key outside its text fields and yields a code editor key past wider scopes', () => {
		const { editor, invoke, invokeShortcut, renameField, row, terminal } = renderScopedShortcuts();

		expect(press(row, 'F2').defaultPrevented).toBe(true);
		expect(invoke).toHaveBeenCalledWith(RENAME.definition, EXPLORER_ACTION_SCOPE);
		expect(press(renameField, 'F2').defaultPrevented).toBe(false);
		expect(press(terminal, 'F2').defaultPrevented).toBe(false);
		expect(invoke).toHaveBeenCalledOnce();

		expect(press(editor, 'Enter', { metaKey: true, shiftKey: true }).defaultPrevented).toBe(false);
		expect(invokeShortcut).not.toHaveBeenCalled();
		expect(press(terminal, 'Enter', { metaKey: true, shiftKey: true }).defaultPrevented).toBe(true);
		expect(invokeShortcut).toHaveBeenCalledWith(
			expect.objectContaining({ shortcutId: 'workbench.panel.active.maximization.toggle' }),
			'keyboard',
		);
	});
});
