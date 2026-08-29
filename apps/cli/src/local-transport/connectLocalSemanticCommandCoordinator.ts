import { readdir } from 'node:fs/promises';
import path from 'node:path';

import { SemanticCommandCoordinatorStatusSchema } from '@orchbee/semantic-command-protocol';
import {
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_LOCATOR_BYTES,
	SemanticCommandLocalEndpointIdSchema,
	SemanticCommandLocalLocatorSchema,
	type SemanticCommandLocalEndpointId,
	type SemanticCommandLocalLocator,
} from '@orchbee/semantic-command-protocol/local-http';

import { SemanticCommandClient } from '../client';
import { LocalHttpSemanticCommandClientPort } from './LocalHttpSemanticCommandClientPort';
import {
	loadSemanticCommandEndpointNativeClient,
	type SemanticCommandEndpointNativeClient,
} from './semantic-command-endpoint-native-client';

const MAX_DISCOVERY_CANDIDATES = 64;
const locatorNamePattern = /^([a-f0-9]{32})\.c\.json$/u;
const locatorDecoder = new TextDecoder('utf-8', { fatal: true });

interface LocalSemanticCommandDiscoveryDependencies {
	readonly readRuntimeDirectory: (runtimeDirectory: string) => Promise<readonly string[]>;
}

const defaultDependencies: LocalSemanticCommandDiscoveryDependencies = {
	readRuntimeDirectory: (runtimeDirectory) => readdir(runtimeDirectory),
};

export interface ConnectLocalSemanticCommandCoordinatorOptions {
	readonly addonPath?: string | undefined;
	readonly endpointId?: SemanticCommandLocalEndpointId | undefined;
	readonly nativeClient?: SemanticCommandEndpointNativeClient | undefined;
	readonly runtimeDirectory: string;
	readonly timeoutMilliseconds?: number | undefined;
}

export type LocalSemanticCommandConnectionResult =
	| { readonly code: 'coordinator-ambiguous' | 'coordinator-unavailable'; readonly kind: 'error' }
	| {
			readonly client: SemanticCommandClient;
			readonly kind: 'connected';
			readonly locator: SemanticCommandLocalLocator;
			readonly port: LocalHttpSemanticCommandClientPort;
	  };

export function connectLocalSemanticCommandCoordinator(
	options: ConnectLocalSemanticCommandCoordinatorOptions,
	dependencies: LocalSemanticCommandDiscoveryDependencies = defaultDependencies,
): Promise<LocalSemanticCommandConnectionResult> {
	return connect(options, dependencies);
}

async function connect(
	options: ConnectLocalSemanticCommandCoordinatorOptions,
	dependencies: LocalSemanticCommandDiscoveryDependencies,
): Promise<LocalSemanticCommandConnectionResult> {
	if (!path.isAbsolute(options.runtimeDirectory)) {
		throw new Error('Semantic command runtime directory must be absolute.');
	}
	if (options.nativeClient !== undefined && options.addonPath !== undefined) {
		throw new Error('Provide either a native client or its addon path, not both.');
	}
	let nativeClient: SemanticCommandEndpointNativeClient;
	if (options.nativeClient !== undefined) {
		nativeClient = options.nativeClient;
	} else if (options.addonPath !== undefined) {
		nativeClient = loadSemanticCommandEndpointNativeClient(options.addonPath);
	} else {
		throw new Error('A semantic command native client is required.');
	}
	const endpointIds = await discoverEndpointIds(options, dependencies);
	if (endpointIds.length === 0) return { kind: 'error', code: 'coordinator-unavailable' };
	if (endpointIds.length > MAX_DISCOVERY_CANDIDATES) {
		return { kind: 'error', code: 'coordinator-ambiguous' };
	}
	const candidates = await Promise.all(
		endpointIds.map((endpointId) => probe(options, nativeClient, endpointId)),
	);
	const connected = candidates.filter(
		(
			candidate,
		): candidate is Extract<LocalSemanticCommandConnectionResult, { kind: 'connected' }> =>
			candidate !== null,
	);
	if (connected.length === 0) return { kind: 'error', code: 'coordinator-unavailable' };
	if (connected.length > 1) return { kind: 'error', code: 'coordinator-ambiguous' };
	const selected = connected[0];
	if (selected === undefined) return { kind: 'error', code: 'coordinator-unavailable' };
	return selected;
}

async function discoverEndpointIds(
	options: ConnectLocalSemanticCommandCoordinatorOptions,
	dependencies: LocalSemanticCommandDiscoveryDependencies,
): Promise<readonly SemanticCommandLocalEndpointId[]> {
	if (options.endpointId !== undefined) {
		return [SemanticCommandLocalEndpointIdSchema.parse(options.endpointId)];
	}
	let names: readonly string[];
	try {
		names = await dependencies.readRuntimeDirectory(options.runtimeDirectory);
	} catch {
		return [];
	}
	return names
		.flatMap((name) => {
			const match = locatorNamePattern.exec(name);
			const endpointId = match?.[1];
			if (endpointId === undefined) return [];
			const parsed = SemanticCommandLocalEndpointIdSchema.safeParse(endpointId);
			return parsed.success ? [parsed.data] : [];
		})
		.sort();
}

async function probe(
	options: ConnectLocalSemanticCommandCoordinatorOptions,
	nativeClient: SemanticCommandEndpointNativeClient,
	endpointId: SemanticCommandLocalEndpointId,
): Promise<Extract<LocalSemanticCommandConnectionResult, { kind: 'connected' }> | null> {
	try {
		const bytes = nativeClient.readLocator(options.runtimeDirectory, endpointId);
		if (
			bytes.byteLength === 0 ||
			bytes.byteLength > SEMANTIC_COMMAND_LOCAL_HTTP_MAX_LOCATOR_BYTES
		) {
			return null;
		}
		const locator = SemanticCommandLocalLocatorSchema.parse(
			JSON.parse(locatorDecoder.decode(bytes)),
		);
		if (locator.endpointId !== endpointId) return null;
		const port = new LocalHttpSemanticCommandClientPort({
			endpointId,
			nativeClient,
			runtimeDirectory: options.runtimeDirectory,
			...(options.timeoutMilliseconds === undefined
				? {}
				: { timeoutMilliseconds: options.timeoutMilliseconds }),
		});
		const status = SemanticCommandCoordinatorStatusSchema.safeParse(await port.readStatus());
		if (!status.success || status.data.coordinatorGeneration !== locator.coordinatorGeneration) {
			return null;
		}
		return {
			client: new SemanticCommandClient({
				coordinatorGeneration: locator.coordinatorGeneration,
				port,
			}),
			kind: 'connected',
			locator,
			port,
		};
	} catch {
		return null;
	}
}
