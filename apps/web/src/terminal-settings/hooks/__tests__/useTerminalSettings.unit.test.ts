// @vitest-environment jsdom

import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { TerminalSettingsController } from '../../model';
import { useTerminalSettings } from '../useTerminalSettings';

type TerminalAppearanceStorage = ConstructorParameters<
	typeof TerminalSettingsController
>[0]['storage'];

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

describe('useTerminalSettings', () => {
	it('projects the controller snapshot through React external-store updates', async () => {
		const controller = new TerminalSettingsController({
			storage: memoryStorage(),
			themeTarget: document.createElement('div'),
		});
		const container = document.createElement('div');
		document.body.append(container);
		const root = createRoot(container);

		await act(async () => {
			root.render(createElement(SnapshotProbe, { controller }));
		});
		expect(container.textContent).toBe('closed:13');

		await act(async () => {
			controller.open();
			controller.update({ fontSize: 17 });
		});
		expect(container.textContent).toBe('open:17');

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
	});
});

function SnapshotProbe({
	controller,
}: {
	readonly controller: TerminalSettingsController;
}): ReactNode {
	const snapshot = useTerminalSettings(controller);
	return `${snapshot.open ? 'open' : 'closed'}:${snapshot.preferences.fontSize}`;
}

function memoryStorage(): TerminalAppearanceStorage {
	return {
		getItem: () => null,
		setItem: () => undefined,
	};
}
