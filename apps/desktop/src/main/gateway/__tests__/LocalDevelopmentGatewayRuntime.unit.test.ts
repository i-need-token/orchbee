import {
	decodeGatewayBrowserFragment,
	GatewayCapabilitySchema,
	GatewayClientConnectionIdSchema,
	GatewayDesktopCommandSchema,
} from '@orchbee/gateway-protocol';
import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
} from '@orchbee/workbench-client-protocol';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	LocalDevelopmentGatewayRuntime,
	type DesktopGatewayClientHandle,
	type DesktopGatewayUtilityProcess,
} from '../LocalDevelopmentGatewayRuntime';

const document = createWorkbenchSharedDocument({
	projectWorkspace: {
		nextProjectSequence: 1,
		nextTerminalContextSequence: 2,
		nextWorkspaceSequence: 1,
		projects: [],
		standaloneTerminalContextId: 'terminal-context-1',
	},
	revision: workbenchSharedDocumentRevision('0'),
	terminalContexts: [
		{
			nextTabSequence: 1,
			panelWorkspace: {
				nextPanelSequence: 1,
				nextPanelSlotSequence: 1,
				nextSplitSequence: 1,
				nextTerminalSequence: 1,
				pendingRemovals: [],
				tabs: [],
			},
			tabCollection: { groups: [], tabs: [] },
			terminalContextId: 'terminal-context-1',
		},
	],
});
const bootstrap = WorkbenchClientSessionBootstrapSchema.parse({
	clientSessionId: 'client-session-1',
	defaultCwd: '/workspace',
	documentState: { document, kind: 'available' },
	initialPresentationId: 'presentation-initial',
	initializationDocument: document,
	sequence: '0',
	terminalHostName: 'orchbee-host',
	terminalEvents: [],
	type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
});

describe('LocalDevelopmentGatewayRuntime', () => {
	it('starts one Utility Process and transfers only scoped page ports', async () => {
		const process = new FakeUtilityProcess();
		const clientSessionPort = fakePort();
		const projectionPort = fakePort();
		let disposedClients = 0;
		const runtime = new LocalDevelopmentGatewayRuntime(
			{
				bindHost: '127.0.0.1',
				entryPath: '/repo/apps/gateway/dist/main/gateway.cjs',
				rendererOrigins: ['http://127.0.0.1:3000'],
			},
			{
				createCapability: () => GatewayCapabilitySchema.parse('a'.repeat(43)),
				createProcess: () => process,
				openClient: async () => ({
					bootstrap,
					clientSessionPort,
					dispose: async () => {
						disposedClients += 1;
					},
					projectionPort,
				}),
			},
		);
		const started = runtime.start();
		process.emitSpawn();
		const startCommand = GatewayDesktopCommandSchema.parse(process.messages[0]?.message);
		expect(startCommand).toMatchObject({
			allowedOrigins: ['http://127.0.0.1:3000'],
			bindHost: '127.0.0.1',
			type: 'gateway-start',
		});

		process.emitMessage({ port: 42_000, type: 'gateway-listening' });
		const { browserUrl } = await started;
		const location = new URL(browserUrl);
		expect(location.searchParams.get('orchbee-runtime')).toBe('gateway');
		expect(decodeGatewayBrowserFragment(location.hash)).toMatchObject({ port: 42_000 });

		const connectionId = GatewayClientConnectionIdSchema.parse('gateway:client:1');
		process.emitMessage({ connectionId, type: 'gateway-client-open-requested' });
		await new Promise<void>((resolve) => setImmediate(resolve));
		const opened = process.messages[1];
		expect(GatewayDesktopCommandSchema.parse(opened?.message)).toMatchObject({
			connectionId,
			type: 'gateway-client-opened',
		});
		expect(opened?.ports).toEqual([clientSessionPort, projectionPort]);

		process.emitMessage({ connectionId, type: 'gateway-client-close-requested' });
		await vi.waitFor(() => expect(disposedClients).toBe(1));
		expect(disposedClients).toBe(1);
		await runtime.dispose();
		expect(process.killed).toBe(true);
	});

	it('kills the Utility Process even when one client retirement rejects, then retries the owner', async () => {
		const process = new FakeUtilityProcess();
		let firstRetirementFailed = true;
		const firstDispose = vi.fn<() => Promise<void>>(async () => {
			if (firstRetirementFailed) throw new Error('first client retirement failed');
		});
		const secondDispose = vi.fn<() => Promise<void>>(async () => undefined);
		const runtime = createRuntime(process, async (connectionId) => ({
			bootstrap,
			clientSessionPort: fakePort(),
			dispose: connectionId.endsWith(':1') ? firstDispose : secondDispose,
			projectionPort: fakePort(),
		}));
		await startRuntime(runtime, process);
		process.emitMessage({
			connectionId: GatewayClientConnectionIdSchema.parse('gateway:client:1'),
			type: 'gateway-client-open-requested',
		});
		process.emitMessage({
			connectionId: GatewayClientConnectionIdSchema.parse('gateway:client:2'),
			type: 'gateway-client-open-requested',
		});
		await vi.waitFor(() => expect(process.messages).toHaveLength(3));

		await expect(runtime.dispose()).rejects.toThrow('did not converge');
		expect(firstDispose).toHaveBeenCalledOnce();
		expect(secondDispose).toHaveBeenCalledOnce();
		expect(process.killed).toBe(true);

		firstRetirementFailed = false;
		await expect(runtime.dispose()).resolves.toBeUndefined();
		expect(firstDispose).toHaveBeenCalledTimes(2);
	});

	it('contains rejected close disposal in the retry owner without an unhandled rejection', async () => {
		const process = new FakeUtilityProcess();
		let shouldReject = true;
		const disposeClient = vi.fn<() => Promise<void>>(async () => {
			if (shouldReject) {
				shouldReject = false;
				throw new Error('close disposal failed');
			}
		});
		const runtime = createRuntime(process, async () => ({
			bootstrap,
			clientSessionPort: fakePort(),
			dispose: disposeClient,
			projectionPort: fakePort(),
		}));
		await startRuntime(runtime, process);
		const unhandled: unknown[] = [];
		const onUnhandled = (reason: unknown): void => {
			unhandled.push(reason);
		};
		globalThis.process.on('unhandledRejection', onUnhandled);
		try {
			const connectionId = GatewayClientConnectionIdSchema.parse('gateway:client:close');
			process.emitMessage({ connectionId, type: 'gateway-client-open-requested' });
			await vi.waitFor(() => expect(process.messages).toHaveLength(2));
			process.emitMessage({ connectionId, type: 'gateway-client-close-requested' });
			await vi.waitFor(() => expect(disposeClient).toHaveBeenCalledTimes(2));
			expect(runtime.backgroundFailure).toBeInstanceOf(Error);
			expect(unhandled).toEqual([]);
		} finally {
			globalThis.process.off('unhandledRejection', onUnhandled);
			await runtime.dispose();
		}
	});

	it('serializes an open/close race and retires a client after transfer failure', async () => {
		const process = new FakeUtilityProcess();
		const clientReady = createDeferred<DesktopGatewayClientHandle>();
		const disposeClient = vi.fn<() => Promise<void>>(async () => undefined);
		const runtime = createRuntime(process, async () => clientReady.promise);
		await startRuntime(runtime, process);
		const connectionId = GatewayClientConnectionIdSchema.parse('gateway:client:race');
		process.emitMessage({ connectionId, type: 'gateway-client-open-requested' });
		process.emitMessage({ connectionId, type: 'gateway-client-close-requested' });
		clientReady.resolve({
			bootstrap,
			clientSessionPort: fakePort(),
			dispose: disposeClient,
			projectionPort: fakePort(),
		});
		await vi.waitFor(() => expect(disposeClient).toHaveBeenCalledOnce());
		expect(process.messages).toHaveLength(2);

		const transferProcess = new FakeUtilityProcess();
		transferProcess.throwOnClientTransfer = true;
		const transferDispose = vi.fn<() => Promise<void>>(async () => undefined);
		const transferRuntime = createRuntime(transferProcess, async () => ({
			bootstrap,
			clientSessionPort: fakePort(),
			dispose: transferDispose,
			projectionPort: fakePort(),
		}));
		await startRuntime(transferRuntime, transferProcess);
		transferProcess.emitMessage({
			connectionId: GatewayClientConnectionIdSchema.parse('gateway:client:transfer'),
			type: 'gateway-client-open-requested',
		});
		await vi.waitFor(() => expect(transferDispose).toHaveBeenCalledOnce());
		expect(transferProcess.killed).toBe(true);
		await transferRuntime.dispose();
		await runtime.dispose();
	});
});

class FakeUtilityProcess implements DesktopGatewayUtilityProcess {
	killed = false;
	throwOnClientTransfer = false;
	readonly messages: Array<{
		readonly message: unknown;
		readonly ports: readonly MessagePortMain[];
	}> = [];
	readonly #exitListeners = new Set<(code: number) => void>();
	readonly #messageListeners = new Set<(message: unknown) => void>();
	readonly #spawnListeners = new Set<() => void>();

	kill(): void {
		this.killed = true;
	}

	onExit(listener: (code: number) => void): () => void {
		this.#exitListeners.add(listener);
		return () => this.#exitListeners.delete(listener);
	}

	onMessage(listener: (message: unknown) => void): () => void {
		this.#messageListeners.add(listener);
		return () => this.#messageListeners.delete(listener);
	}

	onSpawn(listener: () => void): () => void {
		this.#spawnListeners.add(listener);
		return () => this.#spawnListeners.delete(listener);
	}

	postMessage(message: unknown, ports: readonly MessagePortMain[] = []): void {
		const command = GatewayDesktopCommandSchema.safeParse(message);
		if (
			this.throwOnClientTransfer &&
			command.success &&
			command.data.type === 'gateway-client-opened'
		) {
			throw new Error('client transfer failed');
		}
		this.messages.push({ message, ports });
	}

	emitMessage(message: unknown): void {
		for (const listener of this.#messageListeners) listener(message);
	}

	emitSpawn(): void {
		for (const listener of this.#spawnListeners) listener();
	}
}

function createRuntime(
	process: FakeUtilityProcess,
	openClient: NonNullable<
		ConstructorParameters<typeof LocalDevelopmentGatewayRuntime>[1]['openClient']
	>,
): LocalDevelopmentGatewayRuntime {
	return new LocalDevelopmentGatewayRuntime(
		{
			bindHost: '127.0.0.1',
			entryPath: '/repo/apps/gateway/dist/main/gateway.cjs',
			rendererOrigins: ['http://127.0.0.1:3000'],
		},
		{
			createCapability: () => GatewayCapabilitySchema.parse('a'.repeat(43)),
			createProcess: () => process,
			openClient,
		},
	);
}

async function startRuntime(
	runtime: LocalDevelopmentGatewayRuntime,
	process: FakeUtilityProcess,
): Promise<void> {
	const started = runtime.start();
	process.emitSpawn();
	process.emitMessage({ port: 42_000, type: 'gateway-listening' });
	await started;
}

function createDeferred<Value>(): {
	promise: Promise<Value>;
	resolve: (value: Value) => void;
} {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}

function fakePort(): MessagePortMain {
	return { close: () => undefined } as unknown as MessagePortMain;
}
