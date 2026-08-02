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
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import type { BrowserWindow, BrowserWindowConstructorOptions, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { HostConnection } from '../../host';
import type { WorkbenchTopologyStore } from '../../workbench-topology';
import type {
	TerminalDocumentControlCommand,
	TerminalDocumentControlOperationResult,
} from '../../../shared';
import type { DesktopTerminalRuntime } from '../composeDesktopTerminalRuntime';
import { DesktopApplication, type DesktopApplicationDependencies } from '../DesktopApplication';
import { resolveDesktopRuntimeConfiguration } from '../resolveDesktopRuntimeConfiguration';

const identity = {
	creationId: CreationIdSchema.parse('11'.repeat(16)),
	hostEpoch: HostEpochSchema.parse('22'.repeat(16)),
	terminalId: TerminalIdSchema.parse('33'.repeat(16)),
};
const presentationId = terminalPresentationId('presentation-recovered');

describe('DesktopApplication', () => {
	it('starts without creating an attachment and transfers three scoped controls to the sandbox', async () => {
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
		expect(harness.webContents.postMessage).toHaveBeenCalledTimes(3);
		expect(harness.webContents.postMessage).toHaveBeenCalledWith(
			'orchbee.desktop.terminal-document-control-port',
			expect.objectContaining({
				defaultCwd: '/users/orchbee',
				presentations: [{ launchCwd: '/workspace', presentationId }],
			}),
			[expect.anything()],
		);
		expect(harness.webContents.postMessage).toHaveBeenCalledWith(
			'orchbee.desktop.terminal-projection-control-port',
			{ type: 'orchbee.desktop.terminal-projection-control-port' },
			[expect.anything()],
		);
		expect(harness.webContents.postMessage).toHaveBeenCalledWith(
			'orchbee.desktop.workbench-topology-control-port',
			{ type: 'orchbee.desktop.workbench-topology-control-port' },
			[expect.anything()],
		);
		expect(harness.documentBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.projectionBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.topologyBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.createWorkbenchTopologyStore).toHaveBeenCalledOnce();

		await application.shutdown();
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('awaits old document projection retirement before publishing reload controls', async () => {
		let finishRetirement: (() => void) | undefined;
		const harness = createHarness(
			() =>
				new Promise((resolve) => {
					finishRetirement = resolve;
				}),
		);
		const application = createApplication(harness.dependencies);
		await application.start();
		expect(harness.webContents.postMessage).toHaveBeenCalledTimes(3);

		harness.webContents.emit('did-start-loading');
		harness.webContents.emit('did-finish-load');
		await Promise.resolve();
		expect(harness.documentBrokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.projectionBrokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.topologyBrokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.webContents.postMessage).toHaveBeenCalledTimes(3);

		finishRetirement?.();
		await vi.waitFor(() => expect(harness.webContents.postMessage).toHaveBeenCalledTimes(6));
		expect(harness.documentBrokers).toHaveLength(2);
		expect(harness.projectionBrokers).toHaveLength(2);
		expect(harness.topologyBrokers).toHaveLength(2);
		expect(harness.topologyStoresPassedToBrokers[0]).toBe(harness.topologyStoresPassedToBrokers[1]);
		await application.shutdown();
	});

	it('publishes a recovered exit only after the matching provision result is posted', async () => {
		const terminalExit: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('9'),
			identity,
		};
		const harness = createHarness();
		harness.terminalRuntime.getProjectionTarget.mockReturnValue({
			identity,
			readOnly: true,
			terminalExit,
		});
		const application = createApplication(harness.dependencies);
		await application.start();
		const command: TerminalDocumentControlCommand = {
			launchCwd: '/workspace',
			operationId: 'operation-1' as TerminalDocumentControlCommand['operationId'],
			presentationId,
			type: 'terminal-provision-requested',
		};
		const result: TerminalDocumentControlOperationResult = {
			operationId: command.operationId,
			presentationId,
			type: 'terminal-provision-completed',
		};

		harness.resultPosted[0]?.(command, result);

		expect(harness.documentBrokers[0]?.publishTerminalExit).toHaveBeenCalledWith({
			exitCode: 0,
			presentationId,
			signal: null,
			type: 'terminal-exited',
		});
		await application.shutdown();
	});

	it('routes a Host exit through the authoritative presentation registry', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const terminalExit: TerminalExited = {
			exit: { kind: 'signal', signal: 15 },
			finalRevision: CanonicalUint64TextSchema.parse('12'),
			identity,
		};
		harness.terminalRuntime.recordExit.mockResolvedValue(presentationId);

		harness.hostEnvelopeListener?.(terminalExitedEnvelope(terminalExit));

		await vi.waitFor(() => {
			expect(harness.documentBrokers[0]?.publishTerminalExit).toHaveBeenCalledWith({
				exitCode: null,
				presentationId,
				signal: 'SIGTERM',
				type: 'terminal-exited',
			});
		});
		await application.shutdown();
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
						preloadPath: '/preload.cjs',
						rendererFilePath: '/index.html',
						runtime: createRuntime(),
						singleInstanceLockOwned: false,
						traceCopyPath: false,
					},
					harness.dependencies,
				),
		).toThrow('single-instance lock');
	});
});

function createApplication(dependencies: DesktopApplicationDependencies): DesktopApplication {
	return new DesktopApplication(
		{
			preloadPath: '/repo/apps/desktop/dist/preload/preload.cjs',
			rendererFilePath: '/repo/apps/desktop/dist/renderer/index.html',
			runtime: createRuntime(),
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
		loadFile: vi.fn<BrowserWindow['loadFile']>(async () => {
			webContents.emit('did-start-loading');
			webContents.emit('did-finish-load');
		}),
		loadURL: vi.fn<BrowserWindow['loadURL']>().mockResolvedValue(undefined),
		reload: vi.fn<BrowserWindow['reload']>(),
		show: vi.fn<BrowserWindow['show']>(),
		webContents,
	}) as unknown as BrowserWindow;
	let windowOptions: BrowserWindowConstructorOptions | undefined;
	const documentBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly publishTerminalExit: ReturnType<typeof vi.fn<() => boolean>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const projectionBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => Promise<void>>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const topologyBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const topologyStore: WorkbenchTopologyStore = {
		commit: vi
			.fn<WorkbenchTopologyStore['commit']>()
			.mockResolvedValue({ kind: 'unavailable', reason: 'write-failed' }),
		load: vi.fn<WorkbenchTopologyStore['load']>().mockResolvedValue({ kind: 'missing' }),
	};
	const topologyStoresPassedToBrokers: unknown[] = [];
	const resultPosted: DesktopApplicationDependencies['createDocumentBroker'] extends (
		_port: MessagePortMain,
		_operations: infer _Operations,
		result: infer Result,
	) => unknown
		? Result[]
		: never = [];
	const terminalRuntime = {
		dispose: vi.fn<DesktopTerminalRuntime['dispose']>().mockResolvedValue(undefined),
		execute: vi.fn<DesktopTerminalRuntime['execute']>(),
		getProjectionTarget: vi
			.fn<DesktopTerminalRuntime['getProjectionTarget']>()
			.mockReturnValue(null),
		getProjectionMetrics: vi
			.fn<DesktopTerminalRuntime['getProjectionMetrics']>()
			.mockReturnValue([]),
		getRecoveredPresentations: vi
			.fn<DesktopTerminalRuntime['getRecoveredPresentations']>()
			.mockReturnValue([{ launchCwd: '/workspace', presentationId }]),
		initialize: vi.fn<DesktopTerminalRuntime['initialize']>().mockResolvedValue({ kind: 'ready' }),
		open: vi.fn<DesktopTerminalRuntime['open']>(),
		recordExit: vi.fn<DesktopTerminalRuntime['recordExit']>().mockResolvedValue(null),
	};
	const createWindow = vi.fn<DesktopApplicationDependencies['createWindow']>((options) => {
		windowOptions = options;
		return browserWindow;
	});
	let channelSequence = 0;
	const dependencies: DesktopApplicationDependencies = {
		connectHost: vi.fn<DesktopApplicationDependencies['connectHost']>().mockResolvedValue(host),
		createBroker: vi.fn<DesktopApplicationDependencies['createBroker']>(),
		createDocumentBroker: vi.fn<DesktopApplicationDependencies['createDocumentBroker']>(
			(_port, _operations, onResultPosted) => {
				resultPosted.push(onResultPosted);
				const broker = {
					dispose: vi.fn<() => void>(),
					publishTerminalExit: vi.fn<() => boolean>().mockReturnValue(true),
					start: vi.fn<() => void>(),
				};
				documentBrokers.push(broker);
				return broker;
			},
		),
		createMessageChannel: () => ({
			port1: { id: ++channelSequence } as unknown as MessagePortMain,
			port2: { id: ++channelSequence } as unknown as MessagePortMain,
		}),
		createProjectionControlBroker: vi.fn<
			DesktopApplicationDependencies['createProjectionControlBroker']
		>(() => {
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
		createWorkbenchTopologyBroker: vi.fn<
			DesktopApplicationDependencies['createWorkbenchTopologyBroker']
		>((_port, store) => {
			topologyStoresPassedToBrokers.push(store);
			const broker = {
				dispose: vi.fn<() => void>(),
				start: vi.fn<() => void>(),
			};
			topologyBrokers.push(broker);
			return broker;
		}),
		createWorkbenchTopologyStore: vi
			.fn<DesktopApplicationDependencies['createWorkbenchTopologyStore']>()
			.mockReturnValue(topologyStore),
		createWindow,
		randomIdentifier: () => '44'.repeat(16),
		writeTrace: vi.fn<DesktopApplicationDependencies['writeTrace']>(),
	};
	return {
		createWorkbenchTopologyStore: dependencies.createWorkbenchTopologyStore,
		createWindow,
		dependencies,
		documentBrokers,
		host,
		get hostEnvelopeListener() {
			return hostEnvelopeListener;
		},
		projectionBrokers,
		resultPosted,
		terminalRuntime,
		topologyBrokers,
		topologyStoresPassedToBrokers,
		webContents,
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

function createRuntime() {
	return resolveDesktopRuntimeConfiguration({
		applicationPath: '/repo/apps/desktop',
		environment: { SHELL: '/bin/sh' },
		homeDirectory: '/users/orchbee',
		platform: 'linux',
		userDataPath: '/users/orchbee/.config/orchbee',
	});
}
