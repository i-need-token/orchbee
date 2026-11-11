import { describe, expect, it, vi } from 'vitest';

import { memoryUserSettings } from '../../../__tests__/memoryUserSettings';
import { MacosTerminalInputSettingsController } from '../MacosTerminalInputSettingsController';

describe('MacosTerminalInputSettingsController', () => {
	it('defaults to off and publishes one complete live configuration after a saved update', async () => {
		const settings = memoryUserSettings();
		const controller = new MacosTerminalInputSettingsController(settings);
		const listener = vi.fn<Parameters<typeof controller.subscribe>[0]>();
		controller.subscribe(listener);

		expect(controller.current()).toEqual({ optionAsAlt: 'off' });
		controller.update({ optionAsAlt: 'left' });
		await Promise.resolve();
		expect(controller.current()).toEqual({ optionAsAlt: 'left' });
		expect(listener).toHaveBeenCalledWith({ optionAsAlt: 'left' });
		expect(settings.getSnapshot().snapshot?.values['terminal.macos_option_as_alt']).toBe('left');
	});

	it('becomes inert after disposal', () => {
		const controller = new MacosTerminalInputSettingsController(memoryUserSettings());
		controller.dispose();
		expect(controller.update({ optionAsAlt: 'left' })).toEqual({ kind: 'disposed' });
		expect(controller.current()).toEqual({ optionAsAlt: 'off' });
	});
});
