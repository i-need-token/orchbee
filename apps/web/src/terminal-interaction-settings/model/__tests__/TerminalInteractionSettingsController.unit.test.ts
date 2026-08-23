import type { TerminalInteractionConfiguration } from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import { TerminalInteractionSettingsController } from '../TerminalInteractionSettingsController';
import type { TerminalInteractionStorage } from '../terminal-interaction-preferences-storage';

describe('TerminalInteractionSettingsController', () => {
	it('publishes and persists a live copy-on-select transition', () => {
		const storage = createStorage();
		const controller = new TerminalInteractionSettingsController(storage);
		const configurations: TerminalInteractionConfiguration[] = [];
		controller.subscribe((configuration) => configurations.push(configuration));

		expect(controller.current()).toEqual({ copyOnSelect: true });
		expect(controller.update({ copyOnSelect: false })).toEqual({ kind: 'applied-and-saved' });
		expect(controller.getSnapshot()).toEqual({
			persistenceIssue: null,
			preferences: { copyOnSelect: false },
		});
		expect(configurations).toEqual([{ copyOnSelect: false }]);
		expect(storage.setItem).toHaveBeenCalledOnce();
	});

	it('retains the live value and reports a failed durable write', () => {
		const storage = createStorage();
		storage.setItem.mockImplementation(() => {
			throw new Error('injected write failure');
		});
		const controller = new TerminalInteractionSettingsController(storage);

		expect(controller.update({ copyOnSelect: false })).toEqual({ kind: 'applied-session-only' });
		expect(controller.current()).toEqual({ copyOnSelect: false });
		expect(controller.getSnapshot()).toEqual({
			persistenceIssue: 'write-failed',
			preferences: { copyOnSelect: false },
		});
	});

	it('stops publication and mutation after disposal', () => {
		const controller = new TerminalInteractionSettingsController(createStorage());
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);
		controller.dispose();

		expect(controller.update({ copyOnSelect: false })).toEqual({ kind: 'disposed' });
		expect(listener).not.toHaveBeenCalled();
	});
});

function createStorage(): TerminalInteractionStorage & {
	readonly setItem: ReturnType<typeof vi.fn<TerminalInteractionStorage['setItem']>>;
} {
	return {
		getItem: vi.fn<TerminalInteractionStorage['getItem']>(() => null),
		setItem: vi.fn<TerminalInteractionStorage['setItem']>(() => undefined),
	};
}
