import { describe, expect, it, vi } from 'vitest';

import { MacosTerminalInputSettingsController } from '../MacosTerminalInputSettingsController';

describe('MacosTerminalInputSettingsController', () => {
	it('defaults to off and publishes one complete live configuration after a saved update', () => {
		const storage = {
			getItem: vi.fn<() => string | null>(() => null),
			setItem: vi.fn<(key: string, value: string) => void>(),
		};
		const controller = new MacosTerminalInputSettingsController(storage);
		const listener = vi.fn<Parameters<typeof controller.subscribe>[0]>();
		controller.subscribe(listener);

		expect(controller.current()).toEqual({ optionAsAlt: 'off' });
		expect(controller.update({ optionAsAlt: 'left' })).toEqual({ kind: 'applied-and-saved' });
		expect(controller.current()).toEqual({ optionAsAlt: 'left' });
		expect(listener).toHaveBeenCalledWith({ optionAsAlt: 'left' });
		expect(JSON.parse(storage.setItem.mock.calls[0]?.[1] ?? '')).toEqual({
			optionAsAlt: 'left',
		});
	});

	it('keeps a valid session update when storage rejects the write', () => {
		const controller = new MacosTerminalInputSettingsController({
			getItem: () => null,
			setItem: () => {
				throw new Error('full');
			},
		});
		expect(controller.update({ optionAsAlt: 'both' })).toEqual({
			kind: 'applied-session-only',
		});
		expect(controller.getSnapshot()).toEqual({
			persistenceIssue: 'write-failed',
			preferences: { optionAsAlt: 'both' },
		});
	});

	it('publishes reentrant updates in FIFO order without persisting a stale outer value', () => {
		const storage = {
			getItem: () => null,
			setItem: vi.fn<(key: string, value: string) => void>(),
		};
		const controller = new MacosTerminalInputSettingsController(storage);
		const observed: string[] = [];
		controller.subscribe((configuration) => {
			observed.push(`first:${configuration.optionAsAlt}`);
			if (configuration.optionAsAlt === 'left') controller.update({ optionAsAlt: 'right' });
		});
		controller.subscribe((configuration) => observed.push(`second:${configuration.optionAsAlt}`));

		controller.update({ optionAsAlt: 'left' });

		expect(observed).toEqual(['first:left', 'second:left', 'first:right', 'second:right']);
		expect(controller.current()).toEqual({ optionAsAlt: 'right' });
		expect(storage.setItem).toHaveBeenCalledOnce();
		expect(JSON.parse(storage.setItem.mock.calls[0]?.[1] ?? '')).toEqual({
			optionAsAlt: 'right',
		});
	});

	it('becomes inert after disposal', () => {
		const controller = new MacosTerminalInputSettingsController({
			getItem: () => null,
			setItem: () => undefined,
		});
		controller.dispose();
		expect(controller.update({ optionAsAlt: 'left' })).toEqual({ kind: 'disposed' });
		expect(controller.current()).toEqual({ optionAsAlt: 'off' });
	});
});
