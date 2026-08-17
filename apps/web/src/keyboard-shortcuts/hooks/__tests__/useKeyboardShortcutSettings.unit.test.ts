// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { KeyboardShortcutSettingsController } from '../../model';
import { useKeyboardShortcutSettings } from '../useKeyboardShortcutSettings';

afterEach(() => cleanup());

describe('useKeyboardShortcutSettings', () => {
	it('observes the stable controller snapshot', () => {
		const controller = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const rendered = renderHook(() => useKeyboardShortcutSettings(controller));

		act(() => {
			controller.setShortcut('workbench.search.all.open', {
				code: 'KeyG',
				modifiers: ['mod'],
			});
		});
		expect(rendered.result.current.bindings['workbench.search.all.open']).toEqual({
			code: 'KeyG',
			modifiers: ['mod'],
		});
		controller.dispose();
	});
});
