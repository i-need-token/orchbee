import path from 'node:path';
import { parentPort, workerData, type MessagePort } from 'node:worker_threads';

import {
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_HEADER_BYTES,
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES,
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_RESPONSE_BODY_BYTES,
	SemanticCommandLocalEndpointIdSchema,
	SemanticCommandLocalLocatorSchema,
	encodeSemanticCommandLocalHttpResponse,
	inspectSemanticCommandLocalHttpRequest,
	localHttpError,
} from '@orchbee/semantic-command-protocol/local-http';

import { loadSemanticCommandEndpointNativeAdapter } from './semantic-command-endpoint-native-adapter';

const ACCEPT_POLL_MILLISECONDS = 100;
const CONNECTION_DEADLINE_MILLISECONDS = 5_000;
const IO_POLL_MILLISECONDS = 250;
const MAIN_RESPONSE_DEADLINE_MILLISECONDS = 10_000;
const MAX_REQUEST_BYTES =
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_HEADER_BYTES + SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES;
const MAX_RESPONSE_BYTES =
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_HEADER_BYTES +
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_RESPONSE_BODY_BYTES;

interface EndpointWorkerData {
	readonly addonPath: string;
	readonly endpointId: string;
	readonly locatorJson: string;
	readonly runtimeDirectory: string;
}

type ParentMessage =
	| { readonly kind: 'stop' }
	| { readonly bytes: Uint8Array; readonly kind: 'response'; readonly requestId: number };

const options = parseWorkerData(workerData);
const port = requireParentPort(parentPort);
const native = loadSemanticCommandEndpointNativeAdapter(options.addonPath);
let stopping = false;
let nextRequestId = 0;
const responses = new Map<number, (bytes: Uint8Array) => void>();

port.on('message', (message: ParentMessage) => {
	if (message.kind === 'stop') {
		stopping = true;
		return;
	}
	responses.get(message.requestId)?.(message.bytes);
});

let server: object | undefined;
try {
	native.prepareRuntimeDirectory(options.runtimeDirectory);
	server = native.createServer(options.runtimeDirectory, options.endpointId);
	native.publishLocator(
		options.runtimeDirectory,
		options.endpointId,
		Buffer.from(options.locatorJson, 'utf8'),
	);
	port.postMessage({ kind: 'ready' });
	while (!stopping) {
		let accepted = false;
		try {
			accepted = native.acceptBefore(server, ACCEPT_POLL_MILLISECONDS);
		} catch (error) {
			if (!isRejectedConnectionNativeError(error)) throw error;
		}
		if (!accepted) {
			await yieldToWorkerMessages();
			continue;
		}
		try {
			const request = await readRequest(server);
			const response =
				request.kind === 'complete'
					? await requestParent(request.bytes)
					: encodeSemanticCommandLocalHttpResponse(
							rejectionStatus(request.code),
							localHttpError(request.code),
						);
			await writeResponse(server, response);
		} catch (error) {
			if (!isClosedNativeError(error)) throw error;
		} finally {
			native.disconnectServer(server);
		}
		await yieldToWorkerMessages();
	}
} catch (error) {
	port.postMessage({ kind: 'failed', code: nativeErrorCode(error) });
} finally {
	if (server !== undefined) native.closeServer(server);
	try {
		native.removeLocator(options.runtimeDirectory, options.endpointId);
	} catch {
		// A missing or already retired locator is equivalent during teardown.
	}
	port.postMessage({ kind: 'stopped' });
	port.close();
}

async function readRequest(serverHandle: object): Promise<
	| { readonly bytes: Uint8Array; readonly kind: 'complete' }
	| {
			readonly code: 'malformed-request' | 'request-capacity-exceeded' | 'request-timeout';
			readonly kind: 'rejected';
	  }
> {
	const started = performance.now();
	let bytes = Buffer.alloc(0);
	while (performance.now() - started < CONNECTION_DEADLINE_MILLISECONDS) {
		const inspected = inspectSemanticCommandLocalHttpRequest(bytes);
		if (inspected.kind === 'complete') return { bytes, kind: 'complete' };
		if (inspected.kind === 'rejected') {
			return {
				kind: 'rejected',
				code:
					inspected.code === 'request-capacity-exceeded'
						? 'request-capacity-exceeded'
						: 'malformed-request',
			};
		}
		if (bytes.byteLength === MAX_REQUEST_BYTES) {
			return { kind: 'rejected', code: 'request-capacity-exceeded' };
		}
		const chunk = native.readBefore(
			serverHandle,
			Math.min(16 * 1024, MAX_REQUEST_BYTES - bytes.byteLength),
			IO_POLL_MILLISECONDS,
		);
		if (chunk !== null) bytes = Buffer.concat([bytes, Buffer.from(chunk)]);
		await yieldToWorkerMessages();
	}
	return { kind: 'rejected', code: 'request-timeout' };
}

function requestParent(bytes: Uint8Array): Promise<Uint8Array> {
	const requestId = ++nextRequestId;
	return new Promise((resolve) => {
		const timer = setTimeout(() => {
			responses.delete(requestId);
			resolve(
				encodeSemanticCommandLocalHttpResponse(
					503,
					localHttpError('connection-capacity-exhausted'),
				),
			);
		}, MAIN_RESPONSE_DEADLINE_MILLISECONDS);
		responses.set(requestId, (response) => {
			clearTimeout(timer);
			responses.delete(requestId);
			resolve(
				response.byteLength <= MAX_RESPONSE_BYTES
					? response
					: encodeSemanticCommandLocalHttpResponse(
							500,
							localHttpError('response-capacity-exceeded'),
						),
			);
		});
		port.postMessage({ bytes, kind: 'request', requestId });
	});
}

async function writeResponse(serverHandle: object, bytes: Uint8Array): Promise<void> {
	let offset = 0;
	while (offset < bytes.byteLength) {
		offset += native.writeBefore(serverHandle, bytes.subarray(offset), IO_POLL_MILLISECONDS);
		await yieldToWorkerMessages();
	}
}

function yieldToWorkerMessages(): Promise<void> {
	return new Promise((resolve) => setImmediate(resolve));
}

function nativeErrorCode(error: unknown): string {
	if (typeof error !== 'object' || error === null) return 'ERR_ORCHBEE_SEMANTIC_ENDPOINT_SYSTEM';
	const code = Reflect.get(error, 'code');
	return typeof code === 'string' ? code : 'ERR_ORCHBEE_SEMANTIC_ENDPOINT_SYSTEM';
}

function isClosedNativeError(error: unknown): boolean {
	const code = nativeErrorCode(error);
	return (
		code === 'ERR_ORCHBEE_SEMANTIC_ENDPOINT_CONNECTION_CLOSED' ||
		code === 'ERR_ORCHBEE_SEMANTIC_ENDPOINT_TIMEOUT'
	);
}

function rejectionStatus(
	code: 'malformed-request' | 'request-capacity-exceeded' | 'request-timeout',
): 400 | 408 | 413 {
	if (code === 'request-capacity-exceeded') return 413;
	if (code === 'request-timeout') return 408;
	return 400;
}

function isRejectedConnectionNativeError(error: unknown): boolean {
	const code = nativeErrorCode(error);
	return (
		code === 'ERR_ORCHBEE_SEMANTIC_ENDPOINT_CONNECTION_CLOSED' ||
		code === 'ERR_ORCHBEE_SEMANTIC_ENDPOINT_IDENTITY_MISMATCH'
	);
}

function requireParentPort(candidate: MessagePort | null): MessagePort {
	if (candidate === null) throw new Error('Semantic command endpoint worker has no parent port.');
	return candidate;
}

function parseWorkerData(candidate: unknown): EndpointWorkerData {
	if (!isRecord(candidate)) throw new Error('Semantic command endpoint worker data is invalid.');
	const addonPath = candidate['addonPath'];
	const endpointId = candidate['endpointId'];
	const locatorJson = candidate['locatorJson'];
	const runtimeDirectory = candidate['runtimeDirectory'];
	if (
		typeof addonPath !== 'string' ||
		typeof endpointId !== 'string' ||
		typeof locatorJson !== 'string' ||
		typeof runtimeDirectory !== 'string' ||
		!path.isAbsolute(addonPath) ||
		!path.isAbsolute(runtimeDirectory)
	) {
		throw new Error('Semantic command endpoint worker data is invalid.');
	}
	const parsedEndpointId = SemanticCommandLocalEndpointIdSchema.parse(endpointId);
	let locator: unknown;
	try {
		locator = JSON.parse(locatorJson);
	} catch {
		throw new Error('Semantic command endpoint locator is invalid.');
	}
	const parsedLocator = SemanticCommandLocalLocatorSchema.parse(locator);
	if (parsedLocator.endpointId !== parsedEndpointId) {
		throw new Error('Semantic command endpoint locator identity is inconsistent.');
	}
	return { addonPath, endpointId: parsedEndpointId, locatorJson, runtimeDirectory };
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}
