import { desktopSecureKeyboardEntryMessages } from '@orchbee/i18n';
import { describe, expect, it, vi } from 'vitest';

import { DesktopSecureKeyboardEntry } from '../DesktopSecureKeyboardEntry';
import { DesktopSecureKeyboardEntryActions } from '../DesktopSecureKeyboardEntryActions';

describe('DesktopSecureKeyboardEntryActions', () => {
	it('routes the native menu through one checked semantic action', async () => {
		let enabled = false;
		const secureEntry = new DesktopSecureKeyboardEntry('darwin', {
			isEnabled: () => enabled,
			setEnabled: (nextEnabled) => {
				enabled = nextEnabled;
			},
		});
		const actions = new DesktopSecureKeyboardEntryActions(secureEntry);

		expect(actions.getToggleAvailability()).toEqual({ checked: false, kind: 'enabled' });
		expect(actions.invokeToggle('menu')).toBe(true);
		await vi.waitFor(() => {
			expect(actions.getToggleAvailability()).toEqual({ checked: true, kind: 'enabled' });
		});
		actions.dispose();
		secureEntry.dispose();
	});

	it('hides unsupported platforms and disables a failed macOS capability', () => {
		const unsupported = new DesktopSecureKeyboardEntry('linux', {
			isEnabled: () => false,
			setEnabled: () => undefined,
		});
		const unsupportedActions = new DesktopSecureKeyboardEntryActions(unsupported);
		expect(unsupportedActions.getToggleAvailability()).toEqual({ kind: 'hidden' });
		expect(unsupportedActions.invokeToggle('menu')).toBe(false);

		const unavailable = new DesktopSecureKeyboardEntry('darwin', {
			isEnabled: () => {
				throw new Error('unavailable');
			},
			setEnabled: () => undefined,
		});
		const unavailableActions = new DesktopSecureKeyboardEntryActions(unavailable);
		expect(unavailableActions.getToggleAvailability()).toEqual({
			kind: 'disabled',
			reason: desktopSecureKeyboardEntryMessages.unavailable,
		});
		expect(unavailableActions.invokeToggle('menu')).toBe(false);

		unsupportedActions.dispose();
		unsupported.dispose();
		unavailableActions.dispose();
		unavailable.dispose();
	});
});
