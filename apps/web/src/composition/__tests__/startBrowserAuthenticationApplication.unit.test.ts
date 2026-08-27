// @vitest-environment jsdom

import { createRoot } from 'react-dom/client';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { startBrowserAuthenticationApplication } from '../startBrowserAuthenticationApplication';

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

describe('startBrowserAuthenticationApplication', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		Object.defineProperty(window, 'fetch', {
			configurable: true,
			value: vi.fn<typeof fetch>(),
			writable: true,
		});
		document.title = '';
	});

	it('localizes and renders the Personal Access Token sign-in page', () => {
		const rootElement = document.createElement('div');
		const session = startBrowserAuthenticationApplication(
			window,
			rootElement,
			document.documentElement,
			['zh-CN'],
		);

		expect(document.documentElement).toMatchObject({ dir: 'ltr', lang: 'zh-CN' });
		expect(document.title).toBe('登录 — Orchbee');
		expect(createRoot).toHaveBeenCalledWith(rootElement);
		expect(reactRootMocks.render).toHaveBeenCalledOnce();

		session.dispose();
		expect(reactRootMocks.unmount).toHaveBeenCalledOnce();
	});

	it('rejects a missing authentication root before rendering', () => {
		expect(() =>
			startBrowserAuthenticationApplication(window, null, document.documentElement, ['en-US']),
		).toThrow('The Orchbee authentication root element is missing.');
		expect(createRoot).not.toHaveBeenCalled();
	});
});
