import type { TerminalInteractionConfiguration } from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import { memoryUserPreferences } from '../../../__tests__/memoryUserPreferences';
import { TerminalInteractionSettingsController } from '../TerminalInteractionSettingsController';

describe('TerminalInteractionSettingsController', () => {
	it('publishes a confirmed copy-on-select transition live', async () => {
		const controller = new TerminalInteractionSettingsController(
			memoryUserPreferences({ copyOnSelect: true }),
		);
		const configurations: TerminalInteractionConfiguration[] = [];
		controller.subscribe((configuration) => configurations.push(configuration));

		expect(controller.current()).toEqual({ copyOnSelect: true });
		expect(controller.update({ copyOnSelect: false })).toEqual({ kind: 'pending' });
		await Promise.resolve();
		expect(controller.getSnapshot()).toEqual({
			preferences: { copyOnSelect: false, trimTrailingBlankRowsForAgents: true },
		});
		expect(configurations).toEqual([{ copyOnSelect: false }]);
	});

	it('publishes the Agent tail preference without changing terminal interaction configuration', async () => {
		const controller = new TerminalInteractionSettingsController(
			memoryUserPreferences({ copyOnSelect: true }),
		);
		const configurations = vi.fn<(configuration: TerminalInteractionConfiguration) => void>();
		const tailPreference = vi.fn<(enabled: boolean) => void>();
		controller.subscribe(configurations);
		controller.trailingBlankRowsPreferenceSource.subscribe(tailPreference);

		expect(controller.trailingBlankRowsPreferenceSource.current()).toBe(true);
		expect(controller.update({ trimTrailingBlankRowsForAgents: false })).toEqual({
			kind: 'pending',
		});
		await Promise.resolve();
		expect(controller.current()).toEqual({ copyOnSelect: true });
		expect(controller.trailingBlankRowsPreferenceSource.current()).toBe(false);
		expect(configurations).not.toHaveBeenCalled();
		expect(tailPreference).toHaveBeenCalledOnce();
		expect(tailPreference).toHaveBeenCalledWith(false);
	});

	it('stops publication and mutation after disposal', async () => {
		const userPreferences = memoryUserPreferences();
		const controller = new TerminalInteractionSettingsController(userPreferences);
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);
		controller.dispose();

		expect(controller.update({ copyOnSelect: false })).toEqual({ kind: 'disposed' });
		await userPreferences.update({ copyOnSelect: false });
		expect(listener).not.toHaveBeenCalled();
	});
});
