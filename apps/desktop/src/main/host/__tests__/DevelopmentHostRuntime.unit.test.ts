import path from 'node:path';

import { describe, expect, it, vi } from 'vitest';

import type { HostConnection } from '../HostConnection';
import {
	DevelopmentHostRuntime,
	type DevelopmentHostRuntimeDependencies,
} from '../DevelopmentHostRuntime';

describe('DevelopmentHostRuntime', () => {
	it('uses the private platform endpoint shape without treating its ID as a secret', () => {
		const common = {
			binaryPath: path.resolve('/opt/orchbee/terminal-host'),
			endpointId: 'ab'.repeat(16),
			runtimeDirectory: path.resolve('/tmp/orchbee-runtime'),
		};

		expect(new DevelopmentHostRuntime({ ...common, platform: 'darwin' }).endpoint).toBe(
			path.join(common.runtimeDirectory, `terminal-host-${common.endpointId}.sock`),
		);
		expect(new DevelopmentHostRuntime({ ...common, platform: 'win32' }).endpoint).toBe(
			`\\\\.\\pipe\\orchbee-terminal-host-${common.endpointId}`,
		);
	});

	it('launches the explicit binary once and retries the bounded connection', async () => {
		const connection = {} as HostConnection;
		const connect = vi
			.fn<DevelopmentHostRuntimeDependencies['connect']>()
			.mockRejectedValueOnce(new Error('missing endpoint'))
			.mockRejectedValueOnce(new Error('starting'))
			.mockResolvedValueOnce(connection);
		const dependencies: DevelopmentHostRuntimeDependencies = {
			connect,
			launch: vi.fn<DevelopmentHostRuntimeDependencies['launch']>().mockResolvedValue(undefined),
			prepareDirectory: vi
				.fn<DevelopmentHostRuntimeDependencies['prepareDirectory']>()
				.mockResolvedValue(undefined),
			wait: vi.fn<DevelopmentHostRuntimeDependencies['wait']>().mockResolvedValue(undefined),
		};
		const runtime = new DevelopmentHostRuntime(
			{
				binaryPath: path.resolve('/opt/orchbee/terminal-host'),
				connectionAttempts: 2,
				endpointId: 'cd'.repeat(16),
				platform: 'linux',
				retryDelayMilliseconds: 1,
				runtimeDirectory: path.resolve('/tmp/orchbee-runtime'),
			},
			dependencies,
		);

		await expect(runtime.connect()).resolves.toBe(connection);
		expect(dependencies.prepareDirectory).toHaveBeenCalledWith(
			path.resolve('/tmp/orchbee-runtime'),
			'linux',
		);
		expect(dependencies.launch).toHaveBeenCalledOnce();
		expect(connect).toHaveBeenCalledTimes(3);
		expect(dependencies.wait).toHaveBeenCalledWith(1);
	});
});
