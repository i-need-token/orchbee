import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it, vi } from 'vitest';

import type { HostConnection } from '../../host';
import {
	composeDesktopTerminalRuntime,
	type DesktopTerminalRuntimeComposition,
} from '../composeDesktopTerminalRuntime';
import { resolveDesktopRuntimeConfiguration } from '../resolveDesktopRuntimeConfiguration';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

describe('composeDesktopTerminalRuntime', () => {
	it('starts with no eager terminal or projection when the binding ledger is absent', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-runtime-'));
		temporaryDirectories.push(directory);
		const createBroker = vi.fn<DesktopTerminalRuntimeComposition['createBroker']>();
		const createMessageChannel = vi.fn<DesktopTerminalRuntimeComposition['createMessageChannel']>();
		const host = {
			requestControl: vi.fn<HostConnection['requestControl']>(),
		} as unknown as HostConnection;
		const runtime = composeDesktopTerminalRuntime({
			configuration: resolveDesktopRuntimeConfiguration({
				applicationPath: directory,
				environment: { ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: directory },
				homeDirectory: directory,
				platform: 'darwin',
				userDataPath: directory,
			}),
			createBroker,
			createMessageChannel,
			host,
			randomIdentifier: () => '11'.repeat(16),
		});

		await expect(runtime.initialize()).resolves.toEqual({ kind: 'ready' });
		expect(runtime.getRecoveredPresentations()).toEqual([]);
		expect(runtime.getProjectionMetrics()).toEqual([]);
		expect(createBroker).not.toHaveBeenCalled();
		expect(createMessageChannel).not.toHaveBeenCalled();
		await runtime.dispose();
	});
});
