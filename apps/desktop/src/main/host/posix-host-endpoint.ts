import { closeSync } from 'node:fs';
import { createRequire } from 'node:module';
import { Socket } from 'node:net';
import path from 'node:path';
import type { Duplex } from 'node:stream';

import { HostConnection } from './HostConnection';

// Cross-runtime contract: search for `posix-host-endpoint-addon-name` before changing this value.
export const POSIX_HOST_ENDPOINT_ADDON_NAME = 'orchbee-posix-host-endpoint.node';

interface PosixHostEndpointNativeAdapter {
	connectAuthenticated(runtimeDirectory: string, endpointId: string): number;
	prepareRuntimeDirectory(runtimeDirectory: string): void;
}

export interface PosixHostEndpointDependencies {
	readonly closeDescriptor: (descriptor: number) => void;
	readonly createConnection: (transport: Duplex) => HostConnection;
	readonly createSocket: (descriptor: number) => Duplex;
	readonly loadNativeAdapter: (addonPath: string) => PosixHostEndpointNativeAdapter;
}

const defaultDependencies: PosixHostEndpointDependencies = {
	closeDescriptor: (descriptor) => closeSync(descriptor),
	createConnection: (transport) => new HostConnection(transport),
	createSocket: (descriptor) => new Socket({ fd: descriptor, readable: true, writable: true }),
	loadNativeAdapter,
};

export async function preparePosixHostRuntimeDirectory(
	addonPath: string,
	runtimeDirectory: string,
	dependencies: PosixHostEndpointDependencies = defaultDependencies,
): Promise<void> {
	requireAbsoluteAddonPath(addonPath);
	dependencies.loadNativeAdapter(addonPath).prepareRuntimeDirectory(runtimeDirectory);
}

export async function connectAuthenticatedPosixHost(
	addonPath: string,
	runtimeDirectory: string,
	endpointId: string,
	dependencies: PosixHostEndpointDependencies = defaultDependencies,
): Promise<HostConnection> {
	requireAbsoluteAddonPath(addonPath);
	const descriptor = dependencies
		.loadNativeAdapter(addonPath)
		.connectAuthenticated(runtimeDirectory, endpointId);
	let descriptorOwned = true;
	let transport: Duplex | undefined;
	try {
		transport = dependencies.createSocket(descriptor);
		descriptorOwned = false;
		return dependencies.createConnection(transport);
	} catch (error: unknown) {
		if (descriptorOwned) {
			try {
				dependencies.closeDescriptor(descriptor);
			} catch (closeError: unknown) {
				throw new AggregateError(
					[error, closeError],
					'POSIX Terminal Host descriptor handoff and cleanup both failed.',
				);
			}
		} else {
			transport?.destroy();
		}
		throw error;
	}
}

function loadNativeAdapter(addonPath: string): PosixHostEndpointNativeAdapter {
	const candidate: unknown = createRequire(addonPath)(addonPath);
	if (!isRecord(candidate)) {
		throw new Error('POSIX Terminal Host native endpoint adapter is invalid.');
	}
	const connect = candidate['connectAuthenticated'];
	const prepare = candidate['prepareRuntimeDirectory'];
	if (!isUnknownFunction(connect) || !isUnknownFunction(prepare)) {
		throw new Error('POSIX Terminal Host native endpoint adapter exports are invalid.');
	}
	return {
		connectAuthenticated: (runtimeDirectory, endpointId) => {
			const descriptor = connect(runtimeDirectory, endpointId);
			if (
				typeof descriptor !== 'number' ||
				!Number.isInteger(descriptor) ||
				descriptor < 0 ||
				descriptor > 0x7fff_ffff
			) {
				throw new Error('POSIX Terminal Host native adapter returned an invalid descriptor.');
			}
			return descriptor;
		},
		prepareRuntimeDirectory: (runtimeDirectory) => {
			const result = prepare(runtimeDirectory);
			if (result !== undefined) {
				throw new Error(
					'POSIX Terminal Host native adapter returned an invalid preparation result.',
				);
			}
		},
	};
}

function requireAbsoluteAddonPath(addonPath: string): void {
	if (!path.isAbsolute(addonPath)) {
		throw new Error('POSIX Terminal Host native endpoint adapter path must be absolute.');
	}
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}

function isUnknownFunction(
	value: unknown,
): value is (...arguments_: readonly unknown[]) => unknown {
	return typeof value === 'function';
}
