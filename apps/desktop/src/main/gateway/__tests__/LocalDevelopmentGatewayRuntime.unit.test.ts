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
import { describe, expect, it } from 'vitest';

import {
	LocalDevelopmentGatewayRuntime,
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
				entryPath: '/repo/apps/gateway/dist/main/gateway.cjs',
				rendererUrl: 'http://127.0.0.1:3000/',
			},
			{
				createCapability: () => GatewayCapabilitySchema.parse('a'.repeat(43)),
				createProcess: () => process,
				openClient: () => ({
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
			allowedOrigin: 'http://127.0.0.1:3000',
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
		await Promise.resolve();
		expect(disposedClients).toBe(1);
		await runtime.dispose();
		expect(process.killed).toBe(true);
	});
});

class FakeUtilityProcess implements DesktopGatewayUtilityProcess {
	killed = false;
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
		this.messages.push({ message, ports });
	}

	emitMessage(message: unknown): void {
		for (const listener of this.#messageListeners) listener(message);
	}

	emitSpawn(): void {
		for (const listener of this.#spawnListeners) listener();
	}
}

function fakePort(): MessagePortMain {
	return { close: () => undefined } as unknown as MessagePortMain;
}
