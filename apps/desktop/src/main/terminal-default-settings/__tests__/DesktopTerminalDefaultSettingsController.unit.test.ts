import type { TerminalConfiguration } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { DesktopTerminalDefaultSettingsController } from '../DesktopTerminalDefaultSettingsController';
import type {
	TerminalDefaultSettingsStore,
	TerminalDefaultSettingsStoreLoadResult,
} from '../terminal-default-settings-store';

const runtimeConfiguration = { scrollbackBytes: 10_000_000 } as const;

describe('DesktopTerminalDefaultSettingsController', () => {
	it.each([
		{
			expectedIssue: null,
			load: { kind: 'missing' },
		},
		{
			expectedIssue: 'invalid-record',
			load: { kind: 'unavailable', reason: 'invalid-record' },
		},
		{
			expectedIssue: 'read-failed',
			load: { kind: 'unavailable', reason: 'read-failed' },
		},
	] satisfies readonly {
		readonly expectedIssue: null | 'invalid-record' | 'read-failed';
		readonly load: TerminalDefaultSettingsStoreLoadResult;
	}[])('commits the runtime fallback when storage is unavailable', async (expected) => {
		const setHostDefaults = vi.fn<
			(configuration: TerminalConfiguration) => Promise<TerminalConfiguration>
		>(async (configuration) => configuration);
		const controller = createController(storeWith(expected.load), setHostDefaults);

		await expect(controller.initialize()).resolves.toEqual({
			configuration: runtimeConfiguration,
			persistenceIssue: expected.expectedIssue,
			source: 'runtime',
		});
		expect(setHostDefaults).toHaveBeenCalledWith(runtimeConfiguration);
	});

	it('loads a stored value and makes it authoritative in Host before exposing it', async () => {
		const stored = { scrollbackBytes: 25_000_000 };
		const setHostDefaults = vi.fn<
			(configuration: TerminalConfiguration) => Promise<TerminalConfiguration>
		>(async (configuration) => configuration);
		const controller = createController(
			storeWith({ configuration: stored, kind: 'available' }),
			setHostDefaults,
		);

		await expect(controller.initialize()).resolves.toEqual({
			configuration: stored,
			persistenceIssue: null,
			source: 'stored',
		});
		expect(setHostDefaults).toHaveBeenCalledWith(stored);
	});

	it('does not expose startup state when Host rejects the selected default', async () => {
		const controller = createController(storeWith({ kind: 'missing' }), async () => ({
			scrollbackBytes: 5,
		}));

		await expect(controller.initialize()).rejects.toThrow(
			'Terminal Host did not accept the requested terminal defaults.',
		);
		expect(() => controller.getSnapshot()).toThrow(/must initialize/u);
	});

	it('preserves session and durable state when Host rejects an update', async () => {
		const replace = vi.fn<TerminalDefaultSettingsStore['replace']>();
		const setHostDefaults = vi
			.fn<(configuration: TerminalConfiguration) => Promise<TerminalConfiguration>>()
			.mockImplementationOnce(async (configuration) => configuration)
			.mockRejectedValueOnce(new Error('host unavailable'));
		const controller = createController(
			{ load: async () => ({ kind: 'missing' }), replace },
			setHostDefaults,
		);
		await controller.initialize();

		await expect(controller.update({ scrollbackBytes: 20_000_000 })).resolves.toEqual({
			code: 'host-rejected',
			kind: 'failed',
			snapshot: {
				configuration: runtimeConfiguration,
				persistenceIssue: null,
				source: 'runtime',
			},
		});
		expect(replace).not.toHaveBeenCalled();
		expect(controller.getSnapshot().configuration).toEqual(runtimeConfiguration);
	});

	it('keeps a Host-accepted session value visible when persistence fails', async () => {
		const replace = vi.fn<TerminalDefaultSettingsStore['replace']>(async () => {
			throw new Error('disk unavailable');
		});
		const controller = createController(
			{ load: async () => ({ kind: 'missing' }), replace },
			async (configuration) => configuration,
		);
		await controller.initialize();

		await expect(controller.update({ scrollbackBytes: 30_000_000 })).resolves.toEqual({
			kind: 'completed',
			snapshot: {
				configuration: { scrollbackBytes: 30_000_000 },
				persistenceIssue: 'write-failed',
				source: 'session',
			},
		});
		expect(controller.getSnapshot().configuration.scrollbackBytes).toBe(30_000_000);
	});

	it('retries and persists the current session value on a later update', async () => {
		const replace = vi
			.fn<TerminalDefaultSettingsStore['replace']>()
			.mockRejectedValueOnce(new Error('disk unavailable'))
			.mockResolvedValueOnce(undefined);
		const controller = createController(
			{ load: async () => ({ kind: 'missing' }), replace },
			async (configuration) => configuration,
		);
		await controller.initialize();
		await controller.update({ scrollbackBytes: 40_000_000 });

		await expect(controller.update({ scrollbackBytes: 40_000_000 })).resolves.toMatchObject({
			kind: 'completed',
			snapshot: { persistenceIssue: null, source: 'stored' },
		});
		expect(replace).toHaveBeenCalledTimes(2);
	});
});

function createController(
	store: TerminalDefaultSettingsStore,
	setHostDefaults: (configuration: TerminalConfiguration) => Promise<TerminalConfiguration>,
): DesktopTerminalDefaultSettingsController {
	return new DesktopTerminalDefaultSettingsController({
		runtimeConfiguration,
		setHostDefaults,
		store,
	});
}

function storeWith(load: TerminalDefaultSettingsStoreLoadResult): TerminalDefaultSettingsStore {
	return { load: async () => load, replace: async () => undefined };
}
