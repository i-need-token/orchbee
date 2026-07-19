import { createRoot } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';

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
	it('selects the locale before rendering the application', () => {
		const documentElement = { dir: '', lang: '' };
		const rootElement = {} as Element;
		const layoutStorage = {
			getItem: () => null,
			setItem: () => undefined,
		};

		startWebApplication(rootElement, documentElement, ['zh-CN'], 'apple', layoutStorage, [
			{ id: 'workbench', label: 'Workbench' },
		]);

		expect(documentElement).toEqual({ dir: 'ltr', lang: 'zh-CN' });
		expect(createRoot).toHaveBeenCalledWith(rootElement);
		expect(reactRootMocks.render).toHaveBeenCalledOnce();
	});
});
