import { createRequire } from 'node:module';
import path from 'node:path';

// Cross-runtime contract: search for `semantic-command-endpoint-addon-name` before changing this value.
export const SEMANTIC_COMMAND_ENDPOINT_ADDON_NAME = 'orchbee-semantic-command-endpoint.node';

export interface SemanticCommandEndpointNativeClient {
	exchangeAuthenticated(
		runtimeDirectory: string,
		endpointId: string,
		request: Uint8Array,
		responseCapacity: number,
		timeoutMilliseconds: number,
	): Promise<Uint8Array>;
	readLocator(runtimeDirectory: string, endpointId: string): Uint8Array;
}

export function loadSemanticCommandEndpointNativeClient(
	addonPath: string,
): SemanticCommandEndpointNativeClient {
	if (!path.isAbsolute(addonPath)) {
		throw new Error('Semantic command endpoint addon path must be absolute.');
	}
	const candidate: unknown = createRequire(addonPath)(addonPath);
	if (!isRecord(candidate)) {
		throw new Error('Semantic command endpoint native client is invalid.');
	}
	const exchangeAuthenticated = candidate['exchangeAuthenticated'];
	const readLocator = candidate['readLocator'];
	if (typeof exchangeAuthenticated !== 'function' || typeof readLocator !== 'function') {
		throw new Error('Semantic command endpoint native client is invalid.');
	}
	return {
		async exchangeAuthenticated(
			runtimeDirectory,
			endpointId,
			request,
			responseCapacity,
			timeoutMilliseconds,
		) {
			const result: unknown = await Promise.resolve(
				Reflect.apply(exchangeAuthenticated, candidate, [
					runtimeDirectory,
					endpointId,
					request,
					responseCapacity,
					timeoutMilliseconds,
				]),
			);
			if (!(result instanceof Uint8Array)) {
				throw new Error('Semantic command endpoint native exchange result is invalid.');
			}
			return result;
		},
		readLocator(runtimeDirectory, endpointId) {
			const result: unknown = Reflect.apply(readLocator, candidate, [runtimeDirectory, endpointId]);
			if (!(result instanceof Uint8Array)) {
				throw new Error('Semantic command endpoint native locator result is invalid.');
			}
			return result;
		},
	};
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}
