// @vitest-environment jsdom

import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { ApplicationLocaleController } from '../../model';
import { useApplicationLocale } from '../useApplicationLocale';

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

describe('useApplicationLocale', () => {
	it('projects live locale controller snapshots through React', async () => {
		const controller = new ApplicationLocaleController({
			documentElement: { dir: '', lang: '' },
			preferredLocales: ['en-US'],
			storage: {
				getItem: () => null,
				setItem: () => undefined,
			},
		});
		const container = document.createElement('div');
		document.body.append(container);
		const root = createRoot(container);

		await act(async () => root.render(createElement(SnapshotProbe, { controller })));
		expect(container.textContent).toBe('en-US:auto');

		await act(async () => {
			controller.select('zh-CN');
		});
		expect(container.textContent).toBe('zh-CN:zh-CN');

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
	});
});

function SnapshotProbe({
	controller,
}: {
	readonly controller: ApplicationLocaleController;
}): ReactNode {
	const snapshot = useApplicationLocale(controller);
	return `${snapshot.effectiveLocale}:${snapshot.preference ?? 'auto'}`;
}
