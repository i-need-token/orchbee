// @vitest-environment jsdom

import { createRoot } from 'react-dom/client';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { startWebApplication } from '../startWebApplication';

const reactRootMocks = vi.hoisted(() => {
	const render = vi.fn<(node: unknown) => void>();
	const unmount = vi.fn<() => void>();
	return {
		createRoot: vi.fn<
			(rootElement: Element) => { readonly render: typeof render; readonly unmount: typeof unmount }
		>(() => ({ render, unmount })),
		render,
		unmount,
	};
});

vi.mock('react-dom/client', () => ({ createRoot: reactRootMocks.createRoot }));

describe('startWebApplication', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('selects the locale before rendering the application', () => {
		const documentElement = { dataset: {}, dir: '', lang: '' };
		const rootElement = {} as Element;
		const layoutStorage = {
			getItem: () => null,
			setItem: () => undefined,
		};

		const session = startWebApplication(
			rootElement,
			documentElement,
			['zh-CN'],
			'apple',
			layoutStorage,
		);

		expect(documentElement).toEqual({ dataset: { theme: 'light' }, dir: 'ltr', lang: 'zh-CN' });
		expect(createRoot).toHaveBeenCalledWith(rootElement);
		expect(reactRootMocks.render).toHaveBeenCalledOnce();

		session.dispose();
		expect(reactRootMocks.unmount).toHaveBeenCalledOnce();
	});

	it('restores an explicit locale preference instead of browser locale detection', () => {
		const documentElement = { dataset: {}, dir: '', lang: '' };
		const rootElement = {} as Element;
		const layoutStorage = {
			getItem: (key: string) =>
				key === 'orchbee:web:application-locale:v1' ? JSON.stringify({ locale: 'en-US' }) : null,
			setItem: vi.fn<(key: string, value: string) => void>(),
		};

		const session = startWebApplication(
			rootElement,
			documentElement,
			['zh-CN'],
			'apple',
			layoutStorage,
		);

		expect(documentElement.lang).toBe('en-US');
		expect(layoutStorage.setItem).not.toHaveBeenCalledWith(
			'orchbee:web:application-locale:v1',
			expect.any(String),
		);

		session.dispose();
	});

	it('rejects a missing application root before rendering', () => {
		const documentElement = { dataset: {}, dir: '', lang: '' };
		const layoutStorage = {
			getItem: () => null,
			setItem: () => undefined,
		};

		expect(() =>
			startWebApplication(null, documentElement, ['en-US'], 'standard', layoutStorage),
		).toThrow('The Orchbee web root element is missing.');
		expect(createRoot).not.toHaveBeenCalled();
	});
});
