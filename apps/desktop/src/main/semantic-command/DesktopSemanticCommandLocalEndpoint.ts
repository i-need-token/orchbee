import path from 'node:path';
import { Worker, type WorkerOptions } from 'node:worker_threads';

import {
	SEMANTIC_COMMAND_PROTOCOL_VERSION,
	type SemanticCommandCoordinatorGeneration,
} from '@orchbee/semantic-command-protocol';
import {
	SemanticCommandLocalEndpointIdSchema,
	SemanticCommandLocalLocatorSchema,
	type SemanticCommandLocalEndpointId,
} from '@orchbee/semantic-command-protocol/local-http';

import type { DesktopSemanticCommandService } from './DesktopSemanticCommandService';
import { handleDesktopSemanticCommandLocalHttpRequest } from './handleDesktopSemanticCommandLocalHttpRequest';

const ENDPOINT_START_TIMEOUT_MILLISECONDS = 10_000;
const ENDPOINT_STOP_TIMEOUT_MILLISECONDS = 2_000;

interface DesktopSemanticCommandEndpointWorker {
	on(event: 'error', listener: (error: Error) => void): this;
	on(event: 'exit', listener: (code: number) => void): this;
	on(event: 'message', listener: (message: unknown) => void): this;
	postMessage(message: unknown): void;
	terminate(): Promise<number>;
}

interface DesktopSemanticCommandLocalEndpointDependencies {
	readonly createWorker: (
		filename: string,
		options: WorkerOptions,
	) => DesktopSemanticCommandEndpointWorker;
}

const defaultDependencies: DesktopSemanticCommandLocalEndpointDependencies = {
	createWorker: (filename, options) => new Worker(filename, options),
};

export interface DesktopSemanticCommandLocalEndpointOptions {
	readonly addonPath: string;
	readonly coordinatorGeneration: SemanticCommandCoordinatorGeneration;
	readonly endpointId: SemanticCommandLocalEndpointId;
	readonly runtimeDirectory: string;
	readonly service: DesktopSemanticCommandService;
	readonly workerEntryPath: string;
}

export class DesktopSemanticCommandLocalEndpoint {
	readonly #dependencies: DesktopSemanticCommandLocalEndpointDependencies;
	#disposed = false;
	readonly #options: DesktopSemanticCommandLocalEndpointOptions;
	#startPromise: Promise<void> | undefined;
	#worker: DesktopSemanticCommandEndpointWorker | undefined;
	#workerExited = false;
	#workerStopped = false;

	constructor(
		options: DesktopSemanticCommandLocalEndpointOptions,
		dependencies: DesktopSemanticCommandLocalEndpointDependencies = defaultDependencies,
	) {
		if (
			!path.isAbsolute(options.addonPath) ||
			!path.isAbsolute(options.runtimeDirectory) ||
			!path.isAbsolute(options.workerEntryPath)
		) {
			throw new Error('Semantic command endpoint paths must be absolute.');
		}
		this.#options = {
			...options,
			endpointId: SemanticCommandLocalEndpointIdSchema.parse(options.endpointId),
		};
		this.#dependencies = dependencies;
	}

	start(): Promise<void> {
		if (this.#disposed) return Promise.reject(new Error('Semantic command endpoint is disposed.'));
		this.#startPromise ??= this.#start();
		return this.#startPromise;
	}

	async dispose(): Promise<void> {
		if (this.#disposed) return;
		this.#disposed = true;
		const worker = this.#worker;
		if (worker === undefined) return;
		await this.#startPromise?.catch(() => undefined);
		if (this.#workerExited || this.#workerStopped) {
			if (this.#worker === worker) this.#worker = undefined;
			return;
		}
		const stopped = new Promise<void>((resolve) => {
			worker.on('message', (message) => {
				if (isWorkerStopped(message)) resolve();
			});
			worker.on('exit', () => resolve());
		});
		worker.postMessage({ kind: 'stop' });
		const graceful = await Promise.race([
			stopped.then(() => true),
			new Promise<false>((resolve) =>
				setTimeout(() => resolve(false), ENDPOINT_STOP_TIMEOUT_MILLISECONDS),
			),
		]);
		if (!graceful) await worker.terminate();
		if (this.#worker === worker) this.#worker = undefined;
	}

	async #start(): Promise<void> {
		const locator = SemanticCommandLocalLocatorSchema.parse({
			locatorVersion: 1,
			protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
			coordinatorGeneration: this.#options.coordinatorGeneration,
			endpointId: this.#options.endpointId,
		});
		const worker = this.#dependencies.createWorker(this.#options.workerEntryPath, {
			workerData: {
				addonPath: this.#options.addonPath,
				endpointId: this.#options.endpointId,
				locatorJson: JSON.stringify(locator),
				runtimeDirectory: this.#options.runtimeDirectory,
			},
		});
		this.#worker = worker;
		this.#workerExited = false;
		this.#workerStopped = false;
		worker.on('exit', () => {
			this.#workerExited = true;
		});
		worker.on('message', (message) => {
			if (isWorkerStopped(message)) this.#workerStopped = true;
			if (!isWorkerRequest(message)) return;
			const response = handleDesktopSemanticCommandLocalHttpRequest(
				this.#options.service,
				message.bytes,
			);
			worker.postMessage({ bytes: response, kind: 'response', requestId: message.requestId });
		});
		await new Promise<void>((resolve, reject) => {
			let settled = false;
			const finish = (error?: Error): void => {
				if (settled) return;
				settled = true;
				clearTimeout(timer);
				if (error === undefined) resolve();
				else reject(error);
			};
			const timer = setTimeout(
				() => finish(new Error('Semantic command endpoint start timed out.')),
				ENDPOINT_START_TIMEOUT_MILLISECONDS,
			);
			worker.on('error', () => finish(new Error('Semantic command endpoint worker failed.')));
			worker.on('exit', () => finish(new Error('Semantic command endpoint worker exited early.')));
			worker.on('message', (message) => {
				if (isWorkerReady(message)) finish();
				if (isWorkerFailed(message)) {
					finish(new Error(`Semantic command endpoint failed with ${message.code}.`));
				}
			});
		});
	}
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}

function isWorkerReady(value: unknown): boolean {
	return isRecord(value) && value['kind'] === 'ready';
}

function isWorkerStopped(value: unknown): boolean {
	return isRecord(value) && value['kind'] === 'stopped';
}

function isWorkerFailed(
	value: unknown,
): value is { readonly code: string; readonly kind: 'failed' } {
	return isRecord(value) && value['kind'] === 'failed' && typeof value['code'] === 'string';
}

function isWorkerRequest(
	value: unknown,
): value is { readonly bytes: Uint8Array; readonly kind: 'request'; readonly requestId: number } {
	return (
		isRecord(value) &&
		value['kind'] === 'request' &&
		value['bytes'] instanceof Uint8Array &&
		typeof value['requestId'] === 'number' &&
		Number.isSafeInteger(value['requestId']) &&
		value['requestId'] > 0
	);
}
