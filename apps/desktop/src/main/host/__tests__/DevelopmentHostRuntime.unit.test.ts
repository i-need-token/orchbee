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
			binaryPath: path.resolve('/opt/orchbee/orchbee-terminal-host'),
			endpointId: 'ab'.repeat(16),
			runtimeDirectory: path.resolve('/tmp/orchbee-runtime'),
			terminalDefaults: { scrollbackBytes: 10_000_000 },
		};

		expect(new DevelopmentHostRuntime({ ...common, platform: 'darwin' }).endpoint).toBe(
			path.join(common.runtimeDirectory, `${common.endpointId}.s`),
		);
		expect(new DevelopmentHostRuntime({ ...common, platform: 'win32' }).endpoint).toBe(
			`\\\\.\\pipe\\orchbee-terminal-host-${common.endpointId}`,
		);
	});

	it('launches once after an unauthenticated probe and revalidates every bounded retry', async () => {
		const connection = {} as HostConnection;
		const connect = vi
			.fn<DevelopmentHostRuntimeDependencies['connect']>()
			.mockRejectedValueOnce(new Error('peer effective UID does not match'))
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
				binaryPath: path.resolve('/opt/orchbee/orchbee-terminal-host'),
				connectionAttempts: 2,
				endpointId: 'cd'.repeat(16),
				platform: 'linux',
				retryDelayMilliseconds: 1,
				runtimeDirectory: path.resolve('/tmp/orchbee-runtime'),
				terminalDefaults: { scrollbackBytes: 25_000_000 },
			},
			dependencies,
		);

		await expect(runtime.connect()).resolves.toBe(connection);
		expect(dependencies.prepareDirectory).toHaveBeenCalledWith(
			path.resolve('/tmp/orchbee-runtime'),
			'linux',
			path.resolve('/opt/orchbee/orchbee-terminal-host'),
		);
		expect(dependencies.launch).toHaveBeenCalledOnce();
		expect(dependencies.launch).toHaveBeenCalledWith(
			path.resolve('/opt/orchbee/orchbee-terminal-host'),
			[
				'serve',
				'--runtime-directory',
				path.resolve('/tmp/orchbee-runtime'),
				'--endpoint-id',
				'cd'.repeat(16),
				'--scrollback-bytes',
				'25000000',
			],
		);
		expect(connect).toHaveBeenCalledTimes(3);
		expect(connect).toHaveBeenNthCalledWith(
			3,
			path.join(path.resolve('/tmp/orchbee-runtime'), `${'cd'.repeat(16)}.s`),
			path.resolve('/opt/orchbee/orchbee-terminal-host'),
			path.resolve('/tmp/orchbee-runtime'),
			'cd'.repeat(16),
			'linux',
		);
		expect(dependencies.wait).toHaveBeenCalledWith(1);
	});

	it('requests native identity verification again on every Desktop reconnect', async () => {
		const connection = {} as HostConnection;
		const dependencies: DevelopmentHostRuntimeDependencies = {
			connect: vi.fn<DevelopmentHostRuntimeDependencies['connect']>().mockResolvedValue(connection),
			launch: vi.fn<DevelopmentHostRuntimeDependencies['launch']>().mockResolvedValue(undefined),
			prepareDirectory: vi
				.fn<DevelopmentHostRuntimeDependencies['prepareDirectory']>()
				.mockResolvedValue(undefined),
			wait: vi.fn<DevelopmentHostRuntimeDependencies['wait']>().mockResolvedValue(undefined),
		};
		const runtime = new DevelopmentHostRuntime(
			{
				binaryPath: path.resolve('/opt/orchbee/orchbee-terminal-host'),
				endpointId: 'ef'.repeat(16),
				platform: 'darwin',
				runtimeDirectory: path.resolve('/private/runtime'),
				terminalDefaults: { scrollbackBytes: 10_000_000 },
			},
			dependencies,
		);

		await expect(runtime.connect()).resolves.toBe(connection);
		await expect(runtime.connect()).resolves.toBe(connection);
		expect(dependencies.connect).toHaveBeenCalledTimes(2);
		expect(dependencies.prepareDirectory).toHaveBeenCalledTimes(2);
		expect(dependencies.launch).not.toHaveBeenCalled();
	});

	it('retries a supervised Host without launching a competing process', async () => {
		const connection = {} as HostConnection;
		const dependencies: DevelopmentHostRuntimeDependencies = {
			connect: vi
				.fn<DevelopmentHostRuntimeDependencies['connect']>()
				.mockRejectedValueOnce(new Error('supervisor is starting'))
				.mockResolvedValueOnce(connection),
			launch: vi.fn<DevelopmentHostRuntimeDependencies['launch']>().mockResolvedValue(undefined),
			prepareDirectory: vi
				.fn<DevelopmentHostRuntimeDependencies['prepareDirectory']>()
				.mockResolvedValue(undefined),
			wait: vi.fn<DevelopmentHostRuntimeDependencies['wait']>().mockResolvedValue(undefined),
		};
		const runtime = new DevelopmentHostRuntime(
			{
				binaryPath: path.resolve('/opt/orchbee/orchbee-terminal-host'),
				connectionAttempts: 1,
				endpointId: 'ab'.repeat(16),
				launchIfUnavailable: false,
				platform: 'darwin',
				runtimeDirectory: path.resolve('/private/runtime'),
				terminalDefaults: { scrollbackBytes: 10_000_000 },
			},
			dependencies,
		);

		await expect(runtime.connect()).resolves.toBe(connection);
		expect(dependencies.connect).toHaveBeenCalledTimes(2);
		expect(dependencies.launch).not.toHaveBeenCalled();
	});
});
