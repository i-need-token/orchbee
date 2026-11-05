// @vitest-environment jsdom

import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import {
	KEYBOARD_SHORTCUT_CATALOG,
	KeyboardShortcutSettingsController,
} from '../../../../keyboard-shortcuts';
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
		expect(shortcutRows(harness.container)).toHaveLength(KEYBOARD_SHORTCUT_CATALOG.length);

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
		expect(shortcutRows(harness.container)).toHaveLength(KEYBOARD_SHORTCUT_CATALOG.length);

		await changeInput(input, 'no such shortcut');
		expect(shortcutRows(harness.container)).toHaveLength(0);
		expect(harness.container.textContent).toContain('No keyboard shortcuts found.');
		await act(async () => {
			input.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
		});
		expect(shortcutRows(harness.container)).toHaveLength(KEYBOARD_SHORTCUT_CATALOG.length);

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

	it('localizes the discovery controls in Simplified Chinese', async () => {
		const harness = await createHarness('zh-CN');
		expect(harness.container.querySelector("input[aria-label='搜索操作或快捷键']")).not.toBeNull();
		expect(harness.container.textContent).toContain('录制按键');
		await harness.dispose();
	});
});

async function createHarness(locale: OrchbeeLocale) {
	const container = document.createElement('div');
	document.body.append(container);
	const setItem = vi.fn<Storage['setItem']>();
	const controller = new KeyboardShortcutSettingsController({
		platform: 'standard',
		storage: { getItem: () => null, setItem },
	});
	const root = createRoot(container);
	await act(async () => {
		root.render(
			createElement(
				OrchbeeI18nProvider,
				{ locale },
				createElement(KeyboardShortcutSettings, { controller, platform: 'standard' }),
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

function shortcutRows(container: HTMLElement): readonly Element[] {
	return [...container.querySelectorAll('[data-shortcut-id]')];
}
