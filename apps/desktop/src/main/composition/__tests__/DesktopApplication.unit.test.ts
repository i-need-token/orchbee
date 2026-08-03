import { EventEmitter } from 'node:events';

import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	MessageKind,
	PayloadEncoding,
	PROTOCOL_LIMITS,
	TerminalIdSchema,
	type DecodedProtocolEnvelope,
	type TerminalExited,
} from '@orchbee/terminal-protocol';
import { GatewayClientConnectionIdSchema } from '@orchbee/gateway-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench/document';
import type { BrowserWindow, BrowserWindowConstructorOptions, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { HostConnection } from '../../host';
import type { DesktopApplicationLifetimeBrokerDependencies } from '../../application-lifetime';
import {
	DesktopClientSessionBroker,
	DesktopClientSessionCoordinator,
	DesktopTerminalIntentReconciler,
} from '../../client-session';
import type { WorkbenchDocumentStore } from '../../workbench-document';
import type { DesktopTerminalRuntime } from '../composeDesktopTerminalRuntime';
import { DesktopApplication, type DesktopApplicationDependencies } from '../DesktopApplication';
import { resolveDesktopRuntimeConfiguration } from '../resolveDesktopRuntimeConfiguration';

const identity = {
	creationId: CreationIdSchema.parse('11'.repeat(16)),
	hostEpoch: HostEpochSchema.parse('22'.repeat(16)),
	terminalId: TerminalIdSchema.parse('33'.repeat(16)),
};
const presentationId = terminalPresentationId('presentation-recovered');

class TestMainPort extends EventEmitter {
	readonly close = vi.fn<() => void>();
	readonly postMessage = vi.fn<(value: unknown) => void>();
	readonly start = vi.fn<() => void>();
}

describe('DesktopApplication', () => {
	it('starts without creating an attachment and atomically transfers three scoped controls', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);

		await application.start();

		expect(harness.windowOptions?.webPreferences).toMatchObject({
			contextIsolation: true,
			nodeIntegration: false,
			sandbox: true,
			webSecurity: true,
		});
		expect(harness.terminalRuntime.open).not.toHaveBeenCalled();
		expect(harness.browserWindow.loadURL).toHaveBeenCalledWith(
			'http://127.0.0.1:3000/?orchbee-runtime=desktop',
		);
		expect(harness.webContents.postMessage).toHaveBeenCalledOnce();
		expect(harness.webContents.postMessage).toHaveBeenCalledWith(
			'orchbee.desktop.renderer-bootstrap',
			expect.objectContaining({
				clientSession: expect.objectContaining({
					defaultCwd: '/users/orchbee',
					documentState: { kind: 'missing' },
					type: 'orchbee.workbench.client-session-bootstrap',
				}),
				type: 'orchbee.desktop.renderer-bootstrap',
			}),
			[expect.anything(), expect.anything(), expect.anything()],
		);
		expect(harness.clientSessionPorts[0]?.start).toHaveBeenCalledOnce();
		expect(harness.projectionBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.createWorkbenchDocumentStore).toHaveBeenCalledOnce();
		expect(harness.createGatewayRuntime).not.toHaveBeenCalled();

		await application.shutdown();
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('starts the development Gateway with fresh page-scoped controls and no lifetime authority', async () => {
		const harness = createHarness();
		const application = createApplication(
			harness.dependencies,
			'linux',
			createRuntime({
				ORCHBEE_DESKTOP_GATEWAY: '1',
				ORCHBEE_GATEWAY_ENTRY_PATH: '/repo/apps/gateway/dist/main/gateway.cjs',
			}),
		);

		await application.start();

		expect(harness.createGatewayRuntime).toHaveBeenCalledWith(
			{
				entryPath: '/repo/apps/gateway/dist/main/gateway.cjs',
				rendererUrl: 'http://127.0.0.1:3000/',
			},
			{ openClient: expect.any(Function) },
		);
		expect(harness.gatewayRuntime.start).toHaveBeenCalledOnce();
		expect(harness.publishGatewayBrowserUrl).toHaveBeenCalledWith(
			'http://127.0.0.1:3000/?orchbee-runtime=gateway#gateway-capability',
		);
		expect(harness.gatewayRuntime.start.mock.invocationCallOrder[0]).toBeLessThan(
			harness.createWindow.mock.invocationCallOrder[0] ?? Number.POSITIVE_INFINITY,
		);

		const gatewayDependencies = harness.createGatewayRuntime.mock.calls[0]?.[1];
		if (gatewayDependencies === undefined)
			throw new Error('Gateway dependencies were not created.');
		const gatewayClient = gatewayDependencies.openClient(
			GatewayClientConnectionIdSchema.parse('gateway:client:1'),
		);
		expect(harness.clientSessionPorts).toHaveLength(2);
		expect(harness.projectionBrokers).toHaveLength(2);
		expect(harness.lifetimeBrokers).toHaveLength(1);
		const desktopBootstrap = harness.webContents.postMessage.mock.calls[0]?.[1] as
			| { readonly clientSession?: { readonly clientSessionId?: string } }
			| undefined;
		expect(gatewayClient.bootstrap.clientSessionId).not.toBe(
			desktopBootstrap?.clientSession?.clientSessionId,
		);
		await harness.projectionServices[1]?.open(presentationId);
		expect(harness.terminalRuntime.open).toHaveBeenCalledWith(presentationId, {
			brokerEffectTarget: 'client',
		});

		await gatewayClient.dispose();
		await application.shutdown();
		expect(harness.gatewayRuntime.dispose).toHaveBeenCalledOnce();
	});

	it('awaits the old client session attachment retirement before publishing reload controls', async () => {
		let finishRetirement: (() => void) | undefined;
		const harness = createHarness(
			() =>
				new Promise((resolve) => {
					finishRetirement = resolve;
				}),
		);
		const application = createApplication(harness.dependencies);
		await application.start();
		expect(harness.webContents.postMessage).toHaveBeenCalledOnce();

		harness.webContents.emit('did-start-loading');
		harness.webContents.emit('did-finish-load');
		await Promise.resolve();
		expect(harness.clientSessionPorts[0]?.close).toHaveBeenCalledOnce();
		expect(harness.projectionBrokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.webContents.postMessage).toHaveBeenCalledOnce();

		finishRetirement?.();
		await vi.waitFor(() => expect(harness.webContents.postMessage).toHaveBeenCalledTimes(2));
		expect(harness.clientSessionPorts).toHaveLength(2);
		expect(harness.projectionBrokers).toHaveLength(2);
		await application.shutdown();
	});

	it('places recovered terminals in the shared initialization document without eager projection', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();

		const bootstrap = harness.webContents.postMessage.mock.calls.find(
			(call) => call[0] === 'orchbee.desktop.renderer-bootstrap',
		)?.[1] as { readonly clientSession?: unknown } | undefined;
		expect(bootstrap?.clientSession).toMatchObject({
			initializationDocument: {
				terminalContexts: [
					expect.objectContaining({
						panelWorkspace: expect.objectContaining({
							tabs: [
								expect.objectContaining({
									root: expect.objectContaining({
										primaryPresentation: expect.objectContaining({
											intent: { kind: 'retain' },
											presentationId,
										}),
									}),
								}),
							],
						}),
					}),
				],
			},
		});
		expect(harness.terminalRuntime.open).not.toHaveBeenCalled();
		await application.shutdown();
	});

	it('routes a Host exit through the authoritative presentation registry', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const bootstrap = harness.webContents.postMessage.mock.calls.find(
			(call) => call[0] === 'orchbee.desktop.renderer-bootstrap',
		)?.[1] as
			| { readonly clientSession?: { readonly initializationDocument?: unknown } }
			| undefined;
		harness.workbenchDocumentStore.commit.mockResolvedValueOnce({
			kind: 'committed',
			revision: workbenchSharedDocumentRevision('0'),
		});
		harness.clientSessionPorts[0]?.emit('message', {
			data: {
				document: bootstrap?.clientSession?.initializationDocument,
				expectedRevision: null,
				requestId: 'proposal-initial',
				type: 'shared-document-proposed',
			},
			ports: [],
		});
		await vi.waitFor(() => {
			expect(harness.clientSessionPorts[0]?.postMessage).toHaveBeenCalledWith(
				expect.objectContaining({ type: 'shared-document-published' }),
			);
		});
		const terminalExit: TerminalExited = {
			exit: { kind: 'signal', signal: 15 },
			finalRevision: CanonicalUint64TextSchema.parse('12'),
			identity,
		};
		harness.terminalRuntime.recordExit.mockResolvedValue(presentationId);

		harness.hostEnvelopeListener?.(terminalExitedEnvelope(terminalExit));

		await vi.waitFor(() => {
			expect(harness.clientSessionPorts[0]?.postMessage).toHaveBeenCalledWith(
				expect.objectContaining({
					event: {
						exitCode: null,
						presentationId,
						signal: 'SIGTERM',
						type: 'terminal-exited',
					},
					type: 'terminal-presentation-event-published',
				}),
			);
		});
		await application.shutdown();
	});

	it('enters coordinator quiescing before acknowledging an application lifetime mode', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const bootstrap = harness.webContents.postMessage.mock.calls.find(
			(call) => call[0] === 'orchbee.desktop.renderer-bootstrap',
		)?.[1] as
			| { readonly clientSession?: { readonly initializationDocument?: unknown } }
			| undefined;

		expect(harness.lifetimeDependencies[0]?.prepareMode?.('preserve')).toBe(true);
		harness.clientSessionPorts[0]?.emit('message', {
			data: {
				document: bootstrap?.clientSession?.initializationDocument,
				expectedRevision: null,
				requestId: 'proposal-after-quiesce',
				type: 'shared-document-proposed',
			},
			ports: [],
		});

		await vi.waitFor(() => {
			expect(harness.clientSessionPorts[0]?.postMessage).toHaveBeenCalledWith(
				expect.objectContaining({
					result: expect.objectContaining({
						kind: 'rejected',
						reason: 'application-quiescing',
					}),
					type: 'shared-document-proposal-completed',
				}),
			);
		});
		await application.shutdown();
	});

	it('quits only after the scoped lifetime broker authorizes the workflow', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		harness.lifetimeBrokers[0]?.requestExit.mockResolvedValue({
			forced: false,
			kind: 'ready',
			mode: 'preserve',
		});

		await application.requestExitAndQuit('user-quit');

		expect(application.quitPrepared).toBe(true);
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.quitApplication).toHaveBeenCalledOnce();
	});

	it('keeps the last Linux window alive while the user-quit decision is pending', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const preventDefault = vi.fn<() => void>();

		harness.browserWindow.emit('close', { preventDefault });

		expect(preventDefault).toHaveBeenCalledOnce();
		expect(harness.lifetimeBrokers[0]?.requestExit).toHaveBeenCalledWith(
			expect.stringMatching(/^desktop:exit:/u),
			'user-quit',
		);
	});

	it('reopens a closed macOS document before requesting an application-quit decision', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies, 'darwin');
		await application.start();

		harness.browserWindow.emit('closed');
		await application.requestExitAndQuit('user-quit');

		expect(harness.createWindow).toHaveBeenCalledTimes(2);
		expect(harness.lifetimeBrokers).toHaveLength(2);
		expect(harness.lifetimeBrokers[1]?.requestExit).toHaveBeenCalledWith(
			expect.stringMatching(/^desktop:exit:/u),
			'user-quit',
		);
	});

	it('stops startup when the binding registry cannot initialize', async () => {
		const harness = createHarness();
		harness.terminalRuntime.initialize.mockResolvedValue({ kind: 'conflict' });
		const application = createApplication(harness.dependencies);

		await expect(application.start()).rejects.toThrow('initialization stopped with conflict');
		expect(harness.createWindow).not.toHaveBeenCalled();
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('requires ownership of the single-instance lock', () => {
		const harness = createHarness();
		expect(
			() =>
				new DesktopApplication(
					{
						platform: 'linux',
						preloadPath: '/preload.cjs',
						rendererUrl: 'http://127.0.0.1:3000/',
						runtime: createRuntime(),
						singleInstanceLockOwned: false,
						traceCopyPath: false,
					},
					harness.dependencies,
				),
		).toThrow('single-instance lock');
	});
});

function createApplication(
	dependencies: DesktopApplicationDependencies,
	platform: NodeJS.Platform = 'linux',
	runtime = createRuntime(),
): DesktopApplication {
	return new DesktopApplication(
		{
			platform,
			preloadPath: '/repo/apps/desktop/dist/preload/preload.cjs',
			rendererUrl: 'http://127.0.0.1:3000/',
			runtime,
			singleInstanceLockOwned: true,
			traceCopyPath: false,
		},
		dependencies,
	);
}

function createHarness(firstProjectionDispose?: () => Promise<void>) {
	let hostEnvelopeListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
	const host = {
		close: vi.fn<HostConnection['close']>(),
		metrics: {
			bytesReceived: 0,
			bytesSent: 0,
			recordsReceived: 0,
			recordsSent: 0,
			terminalFramesReceived: 0,
		},
		negotiate: vi.fn<HostConnection['negotiate']>().mockResolvedValue({
			frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
			hostEpoch: identity.hostEpoch,
			inputBytes: PROTOCOL_LIMITS.maxInputBytes,
			protocol: { major: 1, minor: 4 },
			rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
			supportedOptionalFeatures: [],
		}),
		onEnvelope: vi.fn<HostConnection['onEnvelope']>((listener) => {
			hostEnvelopeListener = listener;
			return vi.fn<() => void>();
		}),
	} as unknown as HostConnection;
	const webContents = Object.assign(new EventEmitter(), {
		postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
		setWindowOpenHandler: vi.fn<(...arguments_: unknown[]) => void>(),
	});
	const browserWindow = Object.assign(new EventEmitter(), {
		focus: vi.fn<BrowserWindow['focus']>(),
		isDestroyed: vi.fn<BrowserWindow['isDestroyed']>(() => false),
		loadURL: vi.fn<BrowserWindow['loadURL']>(async () => {
			webContents.emit('did-start-loading');
			webContents.emit('did-finish-load');
		}),
		reload: vi.fn<BrowserWindow['reload']>(),
		show: vi.fn<BrowserWindow['show']>(),
		webContents,
	}) as unknown as BrowserWindow;
	let windowOptions: BrowserWindowConstructorOptions | undefined;
	const clientSessionPorts: TestMainPort[] = [];
	const projectionBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => Promise<void>>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const projectionServices: Array<
		Parameters<DesktopApplicationDependencies['createProjectionControlBroker']>[1]
	> = [];
	const lifetimeBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly requestExit: ReturnType<typeof vi.fn>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const lifetimeDependencies: DesktopApplicationLifetimeBrokerDependencies[] = [];
	const workbenchDocumentStore = {
		commit: vi
			.fn<WorkbenchDocumentStore['commit']>()
			.mockResolvedValue({ kind: 'unavailable', reason: 'write-failed' }),
		load: vi.fn<WorkbenchDocumentStore['load']>().mockResolvedValue({ kind: 'missing' }),
	} satisfies WorkbenchDocumentStore;
	const terminalRuntime = {
		closeBindingsNotReferencedBy: vi
			.fn<DesktopTerminalRuntime['closeBindingsNotReferencedBy']>()
			.mockResolvedValue({ closedCount: 0, failures: [] }),
		dispose: vi.fn<DesktopTerminalRuntime['dispose']>().mockResolvedValue(undefined),
		execute: vi.fn<DesktopTerminalRuntime['execute']>(),
		getProjectionTarget: vi
			.fn<DesktopTerminalRuntime['getProjectionTarget']>()
			.mockReturnValue(null),
		getProjectionMetrics: vi
			.fn<DesktopTerminalRuntime['getProjectionMetrics']>()
			.mockReturnValue([]),
		getOwnedBindingCount: vi
			.fn<DesktopTerminalRuntime['getOwnedBindingCount']>()
			.mockReturnValue(0),
		getRecoveredPresentations: vi
			.fn<DesktopTerminalRuntime['getRecoveredPresentations']>()
			.mockReturnValue([{ launchCwd: '/workspace', presentationId }]),
		initialize: vi.fn<DesktopTerminalRuntime['initialize']>().mockResolvedValue({ kind: 'ready' }),
		inspectUnknownHostTerminalCount: vi
			.fn<DesktopTerminalRuntime['inspectUnknownHostTerminalCount']>()
			.mockResolvedValue(0),
		open: vi.fn<DesktopTerminalRuntime['open']>(),
		recordExit: vi.fn<DesktopTerminalRuntime['recordExit']>().mockResolvedValue(null),
	};
	const createWindow = vi.fn<DesktopApplicationDependencies['createWindow']>((options) => {
		windowOptions = options;
		return browserWindow;
	});
	const gatewayRuntime = {
		dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		start: vi.fn<() => Promise<{ readonly browserUrl: string }>>().mockResolvedValue({
			browserUrl: 'http://127.0.0.1:3000/?orchbee-runtime=gateway#gateway-capability',
		}),
	};
	const createGatewayRuntime = vi
		.fn<DesktopApplicationDependencies['createGatewayRuntime']>()
		.mockReturnValue(gatewayRuntime);
	const publishGatewayBrowserUrl =
		vi.fn<DesktopApplicationDependencies['publishGatewayBrowserUrl']>();
	let identifierSequence = 0;
	const dependencies: DesktopApplicationDependencies = {
		connectHost: vi.fn<DesktopApplicationDependencies['connectHost']>().mockResolvedValue(host),
		createApplicationLifetimeBroker: vi.fn<
			DesktopApplicationDependencies['createApplicationLifetimeBroker']
		>((_port, brokerDependencies) => {
			lifetimeDependencies.push(brokerDependencies);
			const broker = {
				dispose: vi.fn<() => void>(),
				requestExit: vi
					.fn<
						ReturnType<
							DesktopApplicationDependencies['createApplicationLifetimeBroker']
						>['requestExit']
					>()
					.mockResolvedValue({ kind: 'cancelled' }),
				start: vi.fn<() => void>(),
			};
			lifetimeBrokers.push(broker);
			return broker;
		}),
		createBroker: vi.fn<DesktopApplicationDependencies['createBroker']>(),
		createClientSessionBroker: vi.fn<DesktopApplicationDependencies['createClientSessionBroker']>(
			(port, coordinator, disposeAttachments) => {
				const mainPort = port as unknown as TestMainPort;
				clientSessionPorts.push(mainPort);
				return new DesktopClientSessionBroker(port, coordinator, { disposeAttachments });
			},
		),
		createClientSessionCoordinator: (coordinatorDependencies) =>
			new DesktopClientSessionCoordinator(coordinatorDependencies),
		createGatewayRuntime,
		createMessageChannel: () => ({
			port1: new TestMainPort() as unknown as MessagePortMain,
			port2: new TestMainPort() as unknown as MessagePortMain,
		}),
		createProjectionControlBroker: vi.fn<
			DesktopApplicationDependencies['createProjectionControlBroker']
		>((_port, service) => {
			projectionServices.push(service);
			const broker = {
				dispose: vi.fn<() => Promise<void>>(
					projectionBrokers.length === 0 && firstProjectionDispose !== undefined
						? firstProjectionDispose
						: () => Promise.resolve(),
				),
				start: vi.fn<() => void>(),
			};
			projectionBrokers.push(broker);
			return broker;
		}),
		createTerminalRuntime: vi
			.fn<DesktopApplicationDependencies['createTerminalRuntime']>()
			.mockReturnValue(terminalRuntime),
		createTerminalIntentReconciler: (reconcilerDependencies) =>
			new DesktopTerminalIntentReconciler(reconcilerDependencies),
		createWorkbenchDocumentStore: vi
			.fn<DesktopApplicationDependencies['createWorkbenchDocumentStore']>()
			.mockReturnValue(workbenchDocumentStore),
		createWindow,
		getPreferredLocales: () => ['en-US'],
		publishGatewayBrowserUrl,
		randomIdentifier: () => (++identifierSequence).toString(16).padStart(32, '0'),
		quitApplication: vi.fn<() => void>(),
		writeTrace: vi.fn<DesktopApplicationDependencies['writeTrace']>(),
	};
	return {
		browserWindow,
		clientSessionPorts,
		createGatewayRuntime,
		createWorkbenchDocumentStore: dependencies.createWorkbenchDocumentStore,
		createWindow,
		dependencies,
		gatewayRuntime,
		host,
		lifetimeBrokers,
		lifetimeDependencies,
		get hostEnvelopeListener() {
			return hostEnvelopeListener;
		},
		projectionBrokers,
		projectionServices,
		publishGatewayBrowserUrl,
		quitApplication: dependencies.quitApplication,
		terminalRuntime,
		webContents,
		workbenchDocumentStore,
		get windowOptions() {
			return windowOptions;
		},
	};
}

function terminalExitedEnvelope(event: TerminalExited): DecodedProtocolEnvelope {
	return {
		correlationId: null,
		event: true,
		hostEpoch: identity.hostEpoch,
		ignoredOptionalFlags: 0,
		kind: MessageKind.terminalExited,
		knownMessage: true,
		optionalMessage: false,
		payload: new TextEncoder().encode(JSON.stringify(event)),
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: { major: 1, minor: 4 },
		response: false,
	};
}

function createRuntime(
	environment: Parameters<typeof resolveDesktopRuntimeConfiguration>[0]['environment'] = {
		SHELL: '/bin/sh',
	},
) {
	return resolveDesktopRuntimeConfiguration({
		applicationPath: '/repo/apps/desktop',
		environment,
		homeDirectory: '/users/orchbee',
		platform: 'linux',
		userDataPath: '/users/orchbee/.config/orchbee',
	});
}
