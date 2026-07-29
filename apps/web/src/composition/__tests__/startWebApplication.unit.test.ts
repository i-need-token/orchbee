import { createRoot } from 'react-dom/client';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { startWebApplication } from '../startWebApplication';

const reactRootMocks = vi.hoisted(() => {
	const render = vi.fn<(node: unknown) => void>();
	return {
		createRoot: vi.fn<(rootElement: Element) => { readonly render: typeof render }>(() => ({
			render,
		})),
		render,
	};
});

vi.mock('react-dom/client', () => ({ createRoot: reactRootMocks.createRoot }));

describe('startWebApplication', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('selects the locale before rendering the application', () => {
		const documentElement = { dir: '', lang: '' };
		const rootElement = {} as Element;
		const layoutStorage = {
			getItem: () => null,
			setItem: () => undefined,
		};

		startWebApplication(rootElement, documentElement, ['zh-CN'], 'apple', layoutStorage);

		expect(documentElement).toEqual({ dir: 'ltr', lang: 'zh-CN' });
		expect(createRoot).toHaveBeenCalledWith(rootElement);
		expect(reactRootMocks.render).toHaveBeenCalledOnce();
	});

	it('rejects a missing application root before rendering', () => {
		const documentElement = { dir: '', lang: '' };
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
