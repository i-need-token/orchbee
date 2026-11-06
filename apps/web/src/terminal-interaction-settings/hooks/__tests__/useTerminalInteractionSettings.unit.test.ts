// @vitest-environment jsdom

import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { memoryUserPreferences } from '../../../__tests__/memoryUserPreferences';
import { TerminalInteractionSettingsController } from '../../model';
import { useTerminalInteractionSettings } from '../useTerminalInteractionSettings';

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

describe('useTerminalInteractionSettings', () => {
	it('follows controller updates', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const root = createRoot(container);

		const controller = new TerminalInteractionSettingsController(
			memoryUserPreferences({ copyOnSelect: true }),
		);
		await act(async () => root.render(createElement(SnapshotProbe, { controller })));
		expect(container.textContent).toBe('enabled');

		await act(async () => {
			controller.update({ copyOnSelect: false });
		});
		expect(container.textContent).toBe('disabled');

		await act(async () => root.unmount());
		container.remove();
		controller.dispose();
	});
});

function SnapshotProbe({
	controller,
}: {
	readonly controller?: TerminalInteractionSettingsController | undefined;
}): ReactNode {
	const snapshot = useTerminalInteractionSettings(controller);
	return snapshot.preferences.copyOnSelect ? 'enabled' : 'disabled';
}
