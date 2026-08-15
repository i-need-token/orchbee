// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
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
	it('closes with Escape when a non-input find control has focus', async () => {
		const controller = new TerminalFindController({
			clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
			updateSearch: vi.fn<TerminalSearchDriver['updateSearch']>(),
		});
		const restoreFocus = vi.fn<() => void>();
		const container = document.createElement('div');
		document.body.appendChild(container);
		const root = createRoot(container);
		controller.open();
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(TerminalFindSurface, {
						controller,
						onNext: vi.fn<() => void>(),
						onPrevious: vi.fn<() => void>(),
						restoreFocus,
					}),
				),
			);
			await Promise.resolve();
		});

		const matchCase = container.querySelector('[aria-label="Match case"]');
		if (!(matchCase instanceof HTMLButtonElement)) {
			throw new Error('Match-case control did not render.');
		}
		matchCase.focus();
		expect(document.activeElement).toBe(matchCase);

		await act(async () => {
			matchCase.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
		});

		expect(container.querySelector('[role="search"]')).toBeNull();
		expect(restoreFocus).toHaveBeenCalledOnce();
		await act(async () => root.unmount());
		container.remove();
	});
});
