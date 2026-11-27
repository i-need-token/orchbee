// @vitest-environment jsdom

import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { KeyboardShortcutSettingsController } from '../../../../keyboard-shortcuts';
import { KeyboardShortcutSettings } from '../KeyboardShortcutSettings';

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

describe('KeyboardShortcutSettings', () => {
	it('finds localized text and typed shortcuts, then clears back to the complete catalog', async () => {
		const harness = await createHarness('en-US');
		const input = requiredElement<HTMLInputElement>(
			harness.container.querySelector("input[aria-label='Search actions or shortcuts']"),
		);
		expect(shortcutRows(harness.container)).toHaveLength(
			Object.keys(harness.controller.current()).length,
		);

		await changeInput(input, 'terminal current');
		expect(shortcutRows(harness.container)).toHaveLength(1);
		expect(harness.container.textContent).toContain('New terminal in current directory');
		expect(harness.container.textContent).toContain('1 shortcut found');

		await changeInput(input, 'ctrl+shift+p');
		expect(shortcutRows(harness.container)).toHaveLength(1);
		expect(harness.container.textContent).toContain('Search actions');

		const clear = requiredElement<HTMLButtonElement>(
			harness.container.querySelector("button[aria-label='Clear shortcut search']"),
		);
		await act(async () => clear.click());
		expect(input.value).toBe('');
		expect(shortcutRows(harness.container)).toHaveLength(
			Object.keys(harness.controller.current()).length,
		);

		await changeInput(input, 'no such shortcut');
		expect(shortcutRows(harness.container)).toHaveLength(0);
		expect(harness.container.textContent).toContain('No keyboard shortcuts found.');
		await act(async () => {
			input.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
		});
		expect(shortcutRows(harness.container)).toHaveLength(
			Object.keys(harness.controller.current()).length,
		);

		await harness.dispose();
	});

	it('records an exact lookup without changing or persisting any binding', async () => {
		const harness = await createHarness('en-US');
		const input = requiredElement<HTMLInputElement>(
			harness.container.querySelector("input[aria-label='Search actions or shortcuts']"),
		);
		const record = requiredElement<HTMLButtonElement>(
			harness.container.querySelector('[data-slot="keyboard-shortcut-record-keys"]'),
		);
		const bindingBefore = harness.controller.current()['workbench.search.actions.open'];

		await act(async () => record.click());
		expect(record.getAttribute('aria-pressed')).toBe('true');
		expect(input.readOnly).toBe(true);
		expect(input.placeholder).toBe('Recording keys — press a shortcut');

		await act(async () => {
			input.dispatchEvent(
				new KeyboardEvent('keydown', {
					bubbles: true,
					cancelable: true,
					code: 'KeyP',
					ctrlKey: true,
					key: 'P',
					shiftKey: true,
				}),
			);
		});
		expect(input.value).toBe('Ctrl+Shift+P');
		expect(shortcutRows(harness.container)).toHaveLength(1);
		expect(harness.container.textContent).toContain('Search actions');
		expect(harness.controller.current()['workbench.search.actions.open']).toBe(bindingBefore);
		expect(harness.setItem).not.toHaveBeenCalled();

		await act(async () => {
			input.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
		});
		expect(record.getAttribute('aria-pressed')).toBe('false');
		expect(input.value).toBe('Ctrl+Shift+P');
		expect(input.readOnly).toBe(false);

		await harness.dispose();
	});

	it('records a File Bar sequence a step at a time and notes the wider keys a line takes', async () => {
		const harness = await createHarness('en-US', { fileBarOffered: true, platform: 'apple' });
		const inFileBarId = 'workbench.file-bar.toggle.in-file-bar';
		const inFileBar = shortcutRecorder(harness.container, inFileBarId);
		expect(document.getElementById(`keyboard-shortcuts.${inFileBarId}.note`)?.textContent).toBe(
			'⌘B runs this instead of Toggle sidebar while focus is there.',
		);
		expect(harness.container.textContent).toContain(
			'When focus is in the File Bar or the file overlay',
		);

		await act(async () => inFileBar.click());
		await pressKey(inFileBar, { code: 'KeyK', key: 'k', metaKey: true });
		expect(inFileBar.textContent).toBe('⌘K, then a key');
		await pressKey(inFileBar, { code: 'KeyW', key: 'w' });
		expect(inFileBar.textContent).toBe('⌘K W');
		expect(harness.setItem.mock.calls.map(([key]) => key)).toEqual([
			'orchbee:web:keyboard-shortcut-overrides:v2',
			'orchbee:web:keyboard-shortcut-overrides:v1',
		]);
		expect(document.getElementById(`keyboard-shortcuts.${inFileBarId}.note`)).toBeNull();

		const windowLine = shortcutRecorder(harness.container, 'workbench.file-bar.toggle');
		await act(async () => windowLine.click());
		await pressKey(windowLine, { code: 'KeyK', key: 'k', metaKey: true });
		expect(windowLine.getAttribute('aria-invalid')).toBe('true');
		expect(harness.container.textContent).toContain('⌘K conflicts with Clear terminal');
		expect(harness.setItem).toHaveBeenCalledTimes(2);
		await harness.dispose();
	});

	it('localizes the discovery controls in Simplified Chinese', async () => {
		const harness = await createHarness('zh-CN');
		expect(harness.container.querySelector("input[aria-label='搜索操作或快捷键']")).not.toBeNull();
		expect(harness.container.textContent).toContain('录制按键');
		await harness.dispose();
	});
});

async function createHarness(
	locale: OrchbeeLocale,
	{
		fileBarOffered = false,
		platform = 'standard',
	}: { readonly fileBarOffered?: boolean; readonly platform?: KeyboardShortcutPlatform } = {},
) {
	const container = document.createElement('div');
	document.body.append(container);
	const setItem = vi.fn<Storage['setItem']>();
	const controller = new KeyboardShortcutSettingsController({
		fileBarOffered,
		platform,
		storage: { getItem: () => null, setItem },
	});
	const root = createRoot(container);
	await act(async () => {
		root.render(
			createElement(
				OrchbeeI18nProvider,
				{ locale },
				createElement(KeyboardShortcutSettings, { controller, platform, query: '' }),
			),
		);
	});
	return {
		container,
		controller,
		dispose: async (): Promise<void> => {
			await act(async () => root.unmount());
			controller.dispose();
			container.remove();
		},
		setItem,
	};
}

async function changeInput(input: HTMLInputElement, value: string): Promise<void> {
	await act(async () => {
		const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
		if (setter === undefined) throw new Error('The input value setter is unavailable.');
		setter.call(input, value);
		input.dispatchEvent(new Event('input', { bubbles: true }));
	});
}

function requiredElement<T extends Element>(element: T | null): T {
	if (element === null) throw new Error('Expected test element.');
	return element;
}

function shortcutRecorder(container: HTMLElement, shortcutId: string): HTMLButtonElement {
	return requiredElement(
		container.querySelector<HTMLButtonElement>(`[data-shortcut-id="${shortcutId}"]`),
	);
}

async function pressKey(
	target: HTMLElement,
	init: { readonly code: string; readonly key: string; readonly metaKey?: boolean },
): Promise<void> {
	await act(async () => {
		target.dispatchEvent(
			new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }),
		);
	});
}

function shortcutRows(container: HTMLElement): readonly Element[] {
	return [...container.querySelectorAll('[data-shortcut-id]')];
}
