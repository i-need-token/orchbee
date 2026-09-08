import { createRequire } from 'node:module';
import path from 'node:path';

// Cross-runtime contract: search for `semantic-command-endpoint-addon-name` before changing this value.
export const SEMANTIC_COMMAND_ENDPOINT_ADDON_NAME = 'orchbee-semantic-command-endpoint.node';

interface SemanticCommandEndpointNativeAdapter {
	acceptBefore(
		server: object,
		pollMilliseconds: number,
		connectionMilliseconds: number,
	): number | null;
	closeServer(server: object): void;
	createServer(runtimeDirectory: string, endpointId: string): object;
	disconnectServer(server: object): void;
	exchangeAuthenticated(
		runtimeDirectory: string,
		endpointId: string,
		request: Uint8Array,
		responseCapacity: number,
		timeoutMilliseconds: number,
	): Promise<Uint8Array>;
	prepareRuntimeDirectory(runtimeDirectory: string): void;
	publishLocator(runtimeDirectory: string, endpointId: string, content: Uint8Array): void;
	readBefore(server: object, capacity: number, timeoutMilliseconds: number): Uint8Array | null;
	readLocator(runtimeDirectory: string, endpointId: string): Uint8Array;
	removeLocator(runtimeDirectory: string, endpointId: string): void;
	writeBefore(server: object, content: Uint8Array, timeoutMilliseconds: number): number;
}

export function loadSemanticCommandEndpointNativeAdapter(
	addonPath: string,
): SemanticCommandEndpointNativeAdapter {
	if (!path.isAbsolute(addonPath)) {
		throw new Error('Semantic command endpoint addon path must be absolute.');
	}
	const candidate: unknown = createRequire(addonPath)(addonPath);
	if (!isRecord(candidate)) {
		throw new Error('Semantic command endpoint native adapter is invalid.');
	}
	for (const name of nativeFunctionNames) requireNativeFunction(candidate, name);
	return {
		acceptBefore(server, pollMilliseconds, connectionMilliseconds) {
			const remaining = invokeNative(candidate, 'acceptBefore', [
				server,
				pollMilliseconds,
				connectionMilliseconds,
			]);
			return remaining === null ? null : requirePositiveInteger(remaining, 'acceptBefore');
		},
		closeServer(server) {
			requireUndefined(invokeNative(candidate, 'closeServer', [server]), 'closeServer');
		},
		createServer(runtimeDirectory, endpointId) {
			return requireObject(
				invokeNative(candidate, 'createServer', [runtimeDirectory, endpointId]),
				'createServer',
			);
		},
		disconnectServer(server) {
			requireUndefined(invokeNative(candidate, 'disconnectServer', [server]), 'disconnectServer');
		},
		async exchangeAuthenticated(
			runtimeDirectory,
			endpointId,
			request,
			responseCapacity,
			timeoutMilliseconds,
		) {
			const result: unknown = await Promise.resolve(
				invokeNative(candidate, 'exchangeAuthenticated', [
					runtimeDirectory,
					endpointId,
					request,
					responseCapacity,
					timeoutMilliseconds,
				]),
			);
			return requireBytes(result, 'exchangeAuthenticated');
		},
		prepareRuntimeDirectory(runtimeDirectory) {
			requireUndefined(
				invokeNative(candidate, 'prepareRuntimeDirectory', [runtimeDirectory]),
				'prepareRuntimeDirectory',
			);
		},
		publishLocator(runtimeDirectory, endpointId, content) {
			requireUndefined(
				invokeNative(candidate, 'publishLocator', [runtimeDirectory, endpointId, content]),
				'publishLocator',
			);
		},
		readBefore(server, capacity, timeoutMilliseconds) {
			const result = invokeNative(candidate, 'readBefore', [server, capacity, timeoutMilliseconds]);
			return result === null ? null : requireBytes(result, 'readBefore');
		},
		readLocator(runtimeDirectory, endpointId) {
			return requireBytes(
				invokeNative(candidate, 'readLocator', [runtimeDirectory, endpointId]),
				'readLocator',
			);
		},
		removeLocator(runtimeDirectory, endpointId) {
			requireUndefined(
				invokeNative(candidate, 'removeLocator', [runtimeDirectory, endpointId]),
				'removeLocator',
			);
		},
		writeBefore(server, content, timeoutMilliseconds) {
			return requirePositiveInteger(
				invokeNative(candidate, 'writeBefore', [server, content, timeoutMilliseconds]),
				'writeBefore',
			);
		},
	};
}

const nativeFunctionNames = [
	'acceptBefore',
	'closeServer',
	'createServer',
	'disconnectServer',
	'exchangeAuthenticated',
	'prepareRuntimeDirectory',
	'publishLocator',
	'readBefore',
	'readLocator',
	'removeLocator',
	'writeBefore',
] as const;

type NativeFunctionName = (typeof nativeFunctionNames)[number];

function invokeNative(
	candidate: Record<string, unknown>,
	name: NativeFunctionName,
	arguments_: readonly unknown[],
): unknown {
	const callable = requireNativeFunction(candidate, name);
	return Reflect.apply(callable, candidate, arguments_);
}

function requireNativeFunction(
	candidate: Record<string, unknown>,
	name: NativeFunctionName,
): (...arguments_: readonly unknown[]) => unknown {
	const callable = candidate[name];
	if (typeof callable !== 'function') {
		throw new Error(`Semantic command endpoint native adapter is missing ${name}.`);
	}
	return (...arguments_: readonly unknown[]) => Reflect.apply(callable, candidate, arguments_);
}

function requireBytes(value: unknown, name: string): Uint8Array {
	if (!(value instanceof Uint8Array)) invalidNativeResult(name);
	return value;
}

function requireObject(value: unknown, name: string): object {
	if (typeof value !== 'object' || value === null) invalidNativeResult(name);
	return value;
}

function requirePositiveInteger(value: unknown, name: string): number {
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value <= 0) {
		invalidNativeResult(name);
	}
	return value;
}

function requireUndefined(value: unknown, name: string): void {
	if (value !== undefined) invalidNativeResult(name);
}

function invalidNativeResult(name: string): never {
	throw new Error(`Semantic command endpoint native adapter returned invalid ${name} data.`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}
