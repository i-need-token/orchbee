import { describe, expect, it, vi } from 'vitest';

import { TerminalInteractionSettingsController } from '../../terminal-interaction-settings';
import { resolveBrowserPreferenceStorage } from '../resolveBrowserPreferenceStorage';

describe('resolveBrowserPreferenceStorage', () => {
	it('returns the browser-owned storage when it is available', () => {
		const storage = {
			getItem: vi.fn<(key: string) => string | null>(() => null),
			setItem: vi.fn<(key: string, value: string) => void>(),
		};

		expect(resolveBrowserPreferenceStorage(() => storage)).toBe(storage);
	});

	it('preserves storage unavailability for feature-owned recovery', () => {
		const storage = resolveBrowserPreferenceStorage(() => {
			throw new Error('Browser storage is unavailable.');
		});
		const controller = new TerminalInteractionSettingsController(storage);

		expect(controller.getSnapshot().persistenceIssue).toBe('read-failed');
		expect(controller.update({ copyOnSelect: false })).toEqual({
			kind: 'applied-session-only',
		});
		expect(controller.getSnapshot().persistenceIssue).toBe('write-failed');
	});
});
