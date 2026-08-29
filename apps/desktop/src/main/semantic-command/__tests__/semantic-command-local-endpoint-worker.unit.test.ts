import { describe, expect, it, vi } from 'vitest';

const fixture = vi.hoisted(() => {
	const listeners: Array<(message: unknown) => void> = [];
	const close = vi.fn<() => void>();
	const postMessage = vi.fn<(message: unknown) => void>((message) => {
		if (
			typeof message === 'object' &&
			message !== null &&
			Reflect.get(message, 'kind') === 'ready'
		) {
			setImmediate(() => {
				for (const listener of listeners) listener({ kind: 'stop' });
			});
		}
	});
	const port = {
		close,
		on: vi.fn<(event: 'message', listener: (message: unknown) => void) => void>(
			(_event, listener) => listeners.push(listener),
		),
		postMessage,
	};
	const server = {};
	const native = {
		acceptBefore: vi.fn<(server: object, timeoutMilliseconds: number) => boolean>(() => false),
		closeServer: vi.fn<(server: object) => void>(),
		createServer: vi.fn<(runtimeDirectory: string, endpointId: string) => object>(() => server),
		disconnectServer: vi.fn<(server: object) => void>(),
		exchangeAuthenticated:
			vi.fn<
				(
					runtimeDirectory: string,
					endpointId: string,
					request: Uint8Array,
					responseCapacity: number,
					timeoutMilliseconds: number,
				) => Promise<Uint8Array>
			>(),
		prepareRuntimeDirectory: vi.fn<(runtimeDirectory: string) => void>(),
		publishLocator:
			vi.fn<(runtimeDirectory: string, endpointId: string, content: Uint8Array) => void>(),
		readBefore:
			vi.fn<(server: object, capacity: number, timeoutMilliseconds: number) => Uint8Array | null>(),
		readLocator: vi.fn<(runtimeDirectory: string, endpointId: string) => Uint8Array>(),
		removeLocator: vi.fn<(runtimeDirectory: string, endpointId: string) => void>(),
		writeBefore:
			vi.fn<(server: object, content: Uint8Array, timeoutMilliseconds: number) => number>(),
	};
	return { native, port, postMessage, close, server };
});

vi.mock('node:worker_threads', () => ({
	parentPort: fixture.port,
	workerData: {
		addonPath: '/opt/orchbee/orchbee-semantic-command-endpoint.node',
		endpointId: '56'.repeat(16),
		locatorJson: JSON.stringify({
			locatorVersion: 1,
			protocolVersion: 1,
			coordinatorGeneration: 'desktop:generation:worker-test',
			endpointId: '56'.repeat(16),
		}),
		runtimeDirectory: '/run/user/501/orchbee',
	},
}));

vi.mock('../semantic-command-endpoint-native-adapter', () => ({
	loadSemanticCommandEndpointNativeAdapter: () => fixture.native,
}));

describe('semantic command local endpoint worker', () => {
	it('publishes only after server creation and retires locator, server, and port on stop', async () => {
		await import('../semantic-command-local-endpoint-worker');

		expect(fixture.native.prepareRuntimeDirectory).toHaveBeenCalledWith('/run/user/501/orchbee');
		expect(fixture.native.createServer).toHaveBeenCalledWith(
			'/run/user/501/orchbee',
			'56'.repeat(16),
		);
		expect(fixture.native.publishLocator).toHaveBeenCalledOnce();
		expect(fixture.postMessage).toHaveBeenCalledWith({ kind: 'ready' });
		expect(fixture.native.closeServer).toHaveBeenCalledWith(fixture.server);
		expect(fixture.native.removeLocator).toHaveBeenCalledWith(
			'/run/user/501/orchbee',
			'56'.repeat(16),
		);
		expect(fixture.postMessage).toHaveBeenCalledWith({ kind: 'stopped' });
		expect(fixture.close).toHaveBeenCalledOnce();
	});
});
