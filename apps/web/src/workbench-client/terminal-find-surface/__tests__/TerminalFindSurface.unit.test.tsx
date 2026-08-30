// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { TerminalSearchMatch } from '@orchbee/terminal';
import { TooltipProvider } from '@orchbee/ui/components';
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { TerminalFindSurface } from '../TerminalFindSurface';
import { TerminalFindController, type TerminalSearchDriver } from '../model/TerminalFindController';

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

describe('TerminalFindSurface', () => {
	it('seeds and selects the active terminal selection, reselects on repeated open, and restores focus on Escape', async () => {
		const controller = createController();
		const restoreFocus = vi.fn<() => void>();
		controller.open('selected terminal text');
		const mounted = await renderSurface(controller, restoreFocus);

		const input = mounted.container.querySelector('input');
		if (!(input instanceof HTMLInputElement)) {
			throw new Error('Find input did not render.');
		}
		expect(input.value).toBe('selected terminal text');
		expect(document.activeElement).toBe(input);
		expect(input.selectionStart).toBe(0);
		expect(input.selectionEnd).toBe(input.value.length);

		input.setSelectionRange(input.value.length, input.value.length);
		await act(async () => {
			controller.open('replacement terminal text');
			await Promise.resolve();
		});
		expect(input.value).toBe('replacement terminal text');
		expect(document.activeElement).toBe(input);
		expect(input.selectionStart).toBe(0);
		expect(input.selectionEnd).toBe(input.value.length);

		const matchCase = mounted.container.querySelector('[aria-label="Match case"]');
		if (!(matchCase instanceof HTMLButtonElement)) {
			throw new Error('Match-case control did not render.');
		}
		input.setSelectionRange(input.value.length, input.value.length);
		matchCase.focus();
		await act(async () => {
			controller.open();
			await Promise.resolve();
		});
		expect(input.value).toBe('replacement terminal text');
		expect(document.activeElement).toBe(input);
		expect(input.selectionStart).toBe(0);
		expect(input.selectionEnd).toBe(input.value.length);
		matchCase.focus();
		await act(async () => {
			matchCase.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1 }));
			await Promise.resolve();
		});
		expect(matchCase.getAttribute('aria-pressed')).toBe('true');
		expect(document.activeElement).toBe(input);

		await act(async () => {
			input.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
			await Promise.resolve();
		});

		expect(mounted.container.querySelector('[role="search"]')).toBeNull();
		expect(restoreFocus).toHaveBeenCalledOnce();
		await mounted.dispose();
	});

	it('closes with Escape when a non-input find control has focus', async () => {
		const controller = createController();
		const restoreFocus = vi.fn<() => void>();
		controller.open();
		const mounted = await renderSurface(controller, restoreFocus);

		const matchCase = mounted.container.querySelector('[aria-label="Match case"]');
		if (!(matchCase instanceof HTMLButtonElement)) {
			throw new Error('Match-case control did not render.');
		}
		matchCase.focus();
		expect(document.activeElement).toBe(matchCase);
		const wholeWord = mounted.container.querySelector('[aria-label="Match whole word"]');
		if (!(wholeWord instanceof HTMLButtonElement)) {
			throw new Error('Whole-word control did not render.');
		}
		matchCase.dispatchEvent(
			new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'ArrowRight' }),
		);
		expect(document.activeElement).toBe(wholeWord);

		await act(async () => {
			wholeWord.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
			await Promise.resolve();
		});

		expect(mounted.container.querySelector('[role="search"]')).toBeNull();
		expect(restoreFocus).toHaveBeenCalledOnce();
		await mounted.dispose();
	});

	it('keeps ordinary result progress compact and expands only actionable search failures', async () => {
		const controller = createController();
		controller.open();
		controller.setQuery('agent');
		controller.applySearchEvent({
			activeIndex: 0,
			activeMatch: matches[0] ?? null,
			errorKind: null,
			generation: 1n,
			matches,
			query: 'agent',
			regexError: null,
			resultLimitReached: false,
			revision: 1n,
			state: 'complete',
		});
		const mounted = await renderSurface(controller, vi.fn<() => void>());

		const input = mounted.container.querySelector('input');
		if (!(input instanceof HTMLInputElement)) {
			throw new Error('Find input did not render.');
		}
		const statusId = input.getAttribute('aria-describedby');
		expect(statusId).toBeTruthy();
		expect(statusId === null ? null : document.getElementById(statusId)?.textContent).toBe(
			'1 of 9 matches',
		);
		expect(
			mounted.container.querySelector('[data-slot="terminal-find-result-count"]')?.textContent,
		).toBe('1/9');
		expect(mounted.container.querySelector('[data-slot="terminal-find-message"]')).toBeNull();
		expect(mounted.container.querySelector('[data-icon="search"]')).toBeNull();
		expect(input.getAttribute('aria-invalid')).toBe('false');

		await act(async () => {
			controller.applySearchEvent({
				activeIndex: -1,
				activeMatch: null,
				errorKind: 'invalid-query',
				generation: 1n,
				matches: [],
				query: 'agent',
				regexError: 'invalid',
				resultLimitReached: false,
				revision: 2n,
				state: 'error',
			});
			await Promise.resolve();
		});

		expect(mounted.container.querySelector('[data-slot="terminal-find-result-count"]')).toBeNull();
		expect(
			mounted.container.querySelector('[data-slot="terminal-find-message"]')?.textContent,
		).toBe('Enter a valid regular expression.');
		expect(input.getAttribute('aria-invalid')).toBe('true');
		await mounted.dispose();
	});
});

const matches: readonly TerminalSearchMatch[] = Array.from({ length: 9 }, (_, index) => ({
	end: { column: index + 5, row: 42 },
	start: { column: index, row: 42 },
	text: 'agent',
}));

function createController(): TerminalFindController {
	return new TerminalFindController({
		clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
		updateSearch: vi.fn<TerminalSearchDriver['updateSearch']>(),
	});
}

async function renderSurface(
	controller: TerminalFindController,
	restoreFocus: () => void,
): Promise<{ readonly container: HTMLElement; dispose(): Promise<void> }> {
	const container = document.createElement('div');
	document.body.appendChild(container);
	const root = createRoot(container);
	await act(async () => {
		root.render(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(
					TooltipProvider,
					undefined,
					createElement(TerminalFindSurface, {
						controller,
						onNext: vi.fn<() => void>(),
						onPrevious: vi.fn<() => void>(),
						restoreFocus,
					}),
				),
			),
		);
		await Promise.resolve();
	});
	return {
		container,
		dispose: () => disposeSurface(root, container),
	};
}

async function disposeSurface(root: Root, container: HTMLElement): Promise<void> {
	await act(async () => root.unmount());
	container.remove();
}
