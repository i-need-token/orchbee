import { EventEmitter } from 'node:events';
import type { WorkerOptions } from 'node:worker_threads';

import { describe, expect, it, vi } from 'vitest';

import { SemanticCommandCoordinatorGenerationSchema } from '@orchbee/semantic-command-protocol';
import {
	decodeSemanticCommandLocalHttpResponse,
	semanticCommandLocalHttpRequests,
	SemanticCommandLocalEndpointIdSchema,
} from '@orchbee/semantic-command-protocol/local-http';

import { DesktopSemanticCommandCatalog } from '../DesktopSemanticCommandCatalog';
import { DesktopSemanticCommandLocalEndpoint } from '../DesktopSemanticCommandLocalEndpoint';
import { DesktopSemanticCommandService } from '../DesktopSemanticCommandService';

class TestWorker extends EventEmitter {
	readonly messages: unknown[] = [];
	readonly postMessage = vi.fn<(message: unknown) => void>((message) => {
		this.messages.push(message);
	});
	readonly terminate = vi.fn<() => Promise<number>>(async () => 0);
}

function createEndpoint(worker: TestWorker) {
	const generation = SemanticCommandCoordinatorGenerationSchema.parse(
		'desktop:generation:endpoint-test',
	);
	const service = new DesktopSemanticCommandService(new DesktopSemanticCommandCatalog([]), {
		coordinatorGeneration: generation,
		createExecutionIdentity: () => {
			throw new Error('The empty catalog cannot allocate an execution.');
		},
	});
	const createWorker = vi.fn<(filename: string, options: WorkerOptions) => TestWorker>(
		() => worker,
	);
	return {
		createWorker,
		endpoint: new DesktopSemanticCommandLocalEndpoint(
			{
				addonPath: '/opt/orchbee/orchbee-semantic-command-endpoint.node',
				coordinatorGeneration: generation,
				endpointId: SemanticCommandLocalEndpointIdSchema.parse('34'.repeat(16)),
				runtimeDirectory: '/run/user/501/orchbee',
				service,
				workerEntryPath: '/opt/orchbee/semantic-command-local-endpoint-worker.cjs',
			},
			{ createWorker },
		),
	};
}

describe('DesktopSemanticCommandLocalEndpoint', () => {
	it('publishes strict locator metadata, delegates one request, and retires the worker', async () => {
		const worker = new TestWorker();
		const { createWorker, endpoint } = createEndpoint(worker);
		const started = endpoint.start();
		worker.emit('message', { kind: 'ready' });
		await started;

		const workerOptions = createWorker.mock.calls[0]?.[1];
		expect(workerOptions?.workerData).toMatchObject({
			endpointId: '34'.repeat(16),
			runtimeDirectory: '/run/user/501/orchbee',
		});
		expect(JSON.parse(String(Reflect.get(workerOptions?.workerData ?? {}, 'locatorJson')))).toEqual(
			{
				locatorVersion: 1,
				protocolVersion: 1,
				coordinatorGeneration: 'desktop:generation:endpoint-test',
				endpointId: '34'.repeat(16),
			},
		);

		worker.emit('message', {
			bytes: semanticCommandLocalHttpRequests.readStatus(),
			kind: 'request',
			requestId: 1,
		});
		const responseMessage = worker.messages.find(
			(message) =>
				typeof message === 'object' &&
				message !== null &&
				Reflect.get(message, 'kind') === 'response',
		);
		const responseBytes = Reflect.get(responseMessage ?? {}, 'bytes');
		expect(responseBytes).toBeInstanceOf(Uint8Array);
		expect(decodeSemanticCommandLocalHttpResponse(responseBytes).body).toMatchObject({
			kind: 'ready',
		});

		const disposed = endpoint.dispose();
		await vi.waitFor(() => expect(worker.postMessage).toHaveBeenCalledWith({ kind: 'stop' }));
		worker.emit('message', { kind: 'stopped' });
		await disposed;
		expect(worker.terminate).not.toHaveBeenCalled();
	});

	it('rejects startup when the worker exits before publishing a locator', async () => {
		const worker = new TestWorker();
		const { endpoint } = createEndpoint(worker);
		const started = endpoint.start();
		worker.emit('exit', 0);

		await expect(started).rejects.toThrow('exited early');
		await endpoint.dispose();
	});
});
