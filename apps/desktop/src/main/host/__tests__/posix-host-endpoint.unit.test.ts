import path from 'node:path';
import { PassThrough } from 'node:stream';
import type { Duplex } from 'node:stream';

import { describe, expect, it, vi } from 'vitest';

import type { HostConnection } from '../HostConnection';
import {
	connectAuthenticatedPosixHost,
	preparePosixHostRuntimeDirectory,
	type PosixHostEndpointDependencies,
} from '../posix-host-endpoint';

const addonPath = path.resolve('/opt/orchbee/orchbee-posix-host-endpoint.node');
const runtimeDirectory = path.resolve('/private/runtime');
const endpointId = 'ab'.repeat(16);

describe('POSIX Host endpoint adapter', () => {
	it('admits no socket or protocol connection after native peer authentication fails', async () => {
		const dependencies = createDependencies();
		dependencies.adapter.connectAuthenticated.mockImplementation(() => {
			throw new Error('POSIX Terminal Host peer effective UID does not match Desktop.');
		});

		await expect(
			connectAuthenticatedPosixHost(addonPath, runtimeDirectory, endpointId, dependencies.value),
		).rejects.toThrow('peer effective UID does not match');
		expect(dependencies.createSocket).not.toHaveBeenCalled();
		expect(dependencies.createConnection).not.toHaveBeenCalled();
		expect(dependencies.closeDescriptor).not.toHaveBeenCalled();
	});

	it('transfers one authenticated descriptor into HostConnection', async () => {
		const dependencies = createDependencies();
		const expected = {} as HostConnection;
		dependencies.adapter.connectAuthenticated.mockReturnValue(41);
		dependencies.createConnection.mockReturnValue(expected);

		await expect(
			connectAuthenticatedPosixHost(addonPath, runtimeDirectory, endpointId, dependencies.value),
		).resolves.toBe(expected);
		expect(dependencies.loadNativeAdapter).toHaveBeenCalledExactlyOnceWith(addonPath);
		expect(dependencies.adapter.connectAuthenticated).toHaveBeenCalledExactlyOnceWith(
			runtimeDirectory,
			endpointId,
		);
		expect(dependencies.createSocket).toHaveBeenCalledExactlyOnceWith(41);
		expect(dependencies.createConnection).toHaveBeenCalledExactlyOnceWith(dependencies.transport);
		expect(dependencies.closeDescriptor).not.toHaveBeenCalled();
	});

	it('closes a descriptor when Node socket construction fails before handoff', async () => {
		const dependencies = createDependencies();
		dependencies.adapter.connectAuthenticated.mockReturnValue(42);
		dependencies.createSocket.mockImplementation(() => {
			throw new Error('injected socket construction failure');
		});

		await expect(
			connectAuthenticatedPosixHost(addonPath, runtimeDirectory, endpointId, dependencies.value),
		).rejects.toThrow('injected socket construction failure');
		expect(dependencies.closeDescriptor).toHaveBeenCalledExactlyOnceWith(42);
		expect(dependencies.createConnection).not.toHaveBeenCalled();
	});

	it('destroys the owning socket when protocol connection construction fails', async () => {
		const dependencies = createDependencies();
		dependencies.adapter.connectAuthenticated.mockReturnValue(43);
		dependencies.createConnection.mockImplementation(() => {
			throw new Error('injected HostConnection construction failure');
		});
		const destroy = vi.spyOn(dependencies.transport, 'destroy');

		await expect(
			connectAuthenticatedPosixHost(addonPath, runtimeDirectory, endpointId, dependencies.value),
		).rejects.toThrow('injected HostConnection construction failure');
		expect(destroy).toHaveBeenCalledOnce();
		expect(dependencies.closeDescriptor).not.toHaveBeenCalled();
	});

	it('prepares the runtime directory inside the native adapter', async () => {
		const dependencies = createDependencies();

		await expect(
			preparePosixHostRuntimeDirectory(addonPath, runtimeDirectory, dependencies.value),
		).resolves.toBeUndefined();
		expect(dependencies.adapter.prepareRuntimeDirectory).toHaveBeenCalledExactlyOnceWith(
			runtimeDirectory,
		);

		dependencies.adapter.prepareRuntimeDirectory.mockImplementation(() => {
			throw new Error('injected insecure runtime directory');
		});
		await expect(
			preparePosixHostRuntimeDirectory(addonPath, runtimeDirectory, dependencies.value),
		).rejects.toThrow('injected insecure runtime directory');
	});

	it('requires a code-resolved absolute native artifact path', async () => {
		const dependencies = createDependencies();

		await expect(
			preparePosixHostRuntimeDirectory(
				'relative-endpoint.node',
				runtimeDirectory,
				dependencies.value,
			),
		).rejects.toThrow('path must be absolute');
		expect(dependencies.loadNativeAdapter).not.toHaveBeenCalled();
	});
});

function createDependencies(): {
	readonly adapter: {
		readonly connectAuthenticated: ReturnType<
			typeof vi.fn<(runtimeDirectory: string, endpointId: string) => number>
		>;
		readonly prepareRuntimeDirectory: ReturnType<typeof vi.fn<(runtimeDirectory: string) => void>>;
	};
	readonly closeDescriptor: ReturnType<typeof vi.fn<(descriptor: number) => void>>;
	readonly createConnection: ReturnType<typeof vi.fn<(transport: Duplex) => HostConnection>>;
	readonly createSocket: ReturnType<typeof vi.fn<(descriptor: number) => PassThrough>>;
	readonly loadNativeAdapter: ReturnType<typeof vi.fn<(addonPath: string) => unknown>>;
	readonly transport: PassThrough;
	readonly value: PosixHostEndpointDependencies;
} {
	const adapter = {
		connectAuthenticated: vi.fn<(runtimeDirectory: string, endpointId: string) => number>(),
		prepareRuntimeDirectory: vi.fn<(runtimeDirectory: string) => void>(),
	};
	const transport = new PassThrough();
	const closeDescriptor = vi.fn<(descriptor: number) => void>();
	const createConnection = vi.fn<(value: Duplex) => HostConnection>();
	const createSocket = vi.fn<(descriptor: number) => PassThrough>(() => transport);
	const loadNativeAdapter = vi.fn<(path: string) => typeof adapter>(() => adapter);
	return {
		adapter,
		closeDescriptor,
		createConnection,
		createSocket,
		loadNativeAdapter,
		transport,
		value: { closeDescriptor, createConnection, createSocket, loadNativeAdapter },
	};
}
