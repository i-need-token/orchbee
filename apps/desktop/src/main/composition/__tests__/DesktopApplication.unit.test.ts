import { EventEmitter } from 'node:events';

import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	PROTOCOL_LIMITS,
	TerminalIdSchema,
	TerminalPhase,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import type { BrowserWindow, BrowserWindowConstructorOptions, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalBrokerMetrics } from '../../broker';
import type { HostConnection } from '../../host';
import type { TerminalBindingLifecycleOutcome } from '../../terminal-lifetime';
import { DesktopApplication, type DesktopApplicationDependencies } from '../DesktopApplication';
import { resolveDesktopRuntimeConfiguration } from '../resolveDesktopRuntimeConfiguration';

const identity = {
	creationId: CreationIdSchema.parse('11'.repeat(16)),
	hostEpoch: HostEpochSchema.parse('22'.repeat(16)),
	terminalId: TerminalIdSchema.parse('33'.repeat(16)),
};

describe('DesktopApplication', () => {
	it('recovers one binding and transfers only a fresh scoped port into a sandboxed window', async () => {
		const harness = createHarness({ identity, kind: 'provisioning', summary: null });
		const application = createApplication(harness.dependencies);

		await application.start();

		expect(harness.windowOptions?.webPreferences).toMatchObject({
			contextIsolation: true,
			nodeIntegration: false,
			sandbox: true,
			webSecurity: true,
		});
		expect(harness.brokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.webContents.postMessage).toHaveBeenCalledWith(
			'orchbee.desktop.projected-attachment-port',
			expect.objectContaining({
				attachmentId: '55'.repeat(16),
				canInput: true,
				terminalExit: null,
			}),
			[expect.anything()],
		);
		expect(harness.requestControl).toHaveBeenCalledOnce();
		await application.shutdown();
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('awaits old attachment retirement before binding a renderer reload generation', async () => {
		let finishRetirement: (() => void) | undefined;
		const harness = createHarness(
			{ identity, kind: 'provisioning', summary: null },
			() =>
				new Promise((resolve) => {
					finishRetirement = resolve;
				}),
		);
		const application = createApplication(harness.dependencies);
		await application.start();
		expect(harness.requestControl).toHaveBeenCalledOnce();

		harness.webContents.emit('did-start-loading');
		harness.webContents.emit('did-finish-load');
		await Promise.resolve();
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.requestControl).toHaveBeenCalledOnce();

		finishRetirement?.();
		await vi.waitFor(() => expect(harness.requestControl).toHaveBeenCalledTimes(2));
		await vi.waitFor(() => expect(harness.brokers).toHaveLength(2));
		const bootstraps = harness.createBroker.mock.calls.map((call) => call[2]);
		expect(bootstraps[0]?.attachmentId).not.toBe(bootstraps[1]?.attachmentId);
		expect(bootstraps[0]?.terminalId).toBe(bootstraps[1]?.terminalId);
		await application.shutdown();
	});

	it('reattaches an exited recovery binding read-only with its retained final revision', async () => {
		const summary: TerminalSummary = {
			columns: 80,
			exit: { code: 0, kind: 'exit-code' },
			identity,
			phase: TerminalPhase.exited,
			revision: CanonicalUint64TextSchema.parse('9'),
			rows: 24,
			terminateRequested: false,
		};
		const harness = createHarness({ identity, kind: 'recovered', summary }, undefined, true);
		const application = createApplication(harness.dependencies);

		await application.start();

		expect(harness.requestControl.mock.calls[0]?.[1]).toMatchObject({
			capabilities: { input: false, read: true, resize: false },
			requestResizeLease: false,
		});
		expect(harness.createBroker.mock.calls[0]?.[2]).toMatchObject({
			canInput: false,
			canResize: false,
			terminalExit: {
				exit: summary.exit,
				finalRevision: summary.revision,
				identity,
			},
		});
		await application.shutdown();
	});

	it('rejects Host capabilities for a foreign attachment identity', async () => {
		const harness = createHarness({ identity, kind: 'provisioning', summary: null });
		harness.requestControl.mockResolvedValueOnce({
			attachmentId: 'aa'.repeat(16),
			capabilities: { input: true, read: true, resize: true },
			resizeLease: true,
			terminalId: identity.terminalId,
		});
		const application = createApplication(harness.dependencies);

		await expect(application.start()).rejects.toThrow('required local attachment capabilities');
		expect(harness.createBroker).not.toHaveBeenCalled();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('retires the scoped broker before requesting close and blocks later reattachment', async () => {
		const harness = createHarness({ identity, kind: 'provisioning', summary: null });
		const application = createApplication(harness.dependencies);
		await application.start();
		harness.terminalLifecycle.requestClose.mockImplementationOnce(async (retireProjection) => {
			await retireProjection();
			expect(harness.brokers[0]?.dispose).toHaveBeenCalledOnce();
			return { kind: 'closed' };
		});

		expect(await application.requestTerminalClose()).toEqual({ kind: 'closed' });
		harness.webContents.emit('did-start-loading');
		harness.webContents.emit('did-finish-load');
		await Promise.resolve();

		expect(harness.requestControl).toHaveBeenCalledOnce();
		expect(harness.createBroker).toHaveBeenCalledOnce();
		await application.shutdown();
	});

	it('stops startup after completing a previously durable close', async () => {
		const harness = createHarness({ kind: 'close-completed' });
		const application = createApplication(harness.dependencies);

		await expect(application.start()).rejects.toThrow('close-completed');
		expect(harness.createWindow).not.toHaveBeenCalled();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('rejects recovery composition without ownership of the single-instance lock', () => {
		const harness = createHarness({ identity, kind: 'provisioning', summary: null });
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

function createHarness(
	outcome: TerminalBindingLifecycleOutcome,
	firstDispose?: () => Promise<void>,
	readOnly = false,
) {
	const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(
		async (_kind, value) => ({
			attachmentId: (value as { attachmentId: string }).attachmentId,
			capabilities: { input: !readOnly, read: true, resize: !readOnly },
			resizeLease: !readOnly,
			terminalId: identity.terminalId,
		}),
	);
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
		onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
		requestControl,
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
	const brokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => Promise<void>>>;
		readonly metrics: ProjectedTerminalBrokerMetrics;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const identifiers = ['44'.repeat(16), '55'.repeat(16), '66'.repeat(16), '77'.repeat(16)];
	const createBroker = vi.fn<DesktopApplicationDependencies['createBroker']>(() => {
		const broker = {
			dispose: vi.fn<() => Promise<void>>(
				brokers.length === 0 && firstDispose !== undefined ? firstDispose : () => Promise.resolve(),
			),
			metrics: createMetrics(),
			start: vi.fn<() => void>(),
		};
		brokers.push(broker);
		return broker;
	});
	const terminalLifecycle = {
		reconcile: vi.fn<() => Promise<TerminalBindingLifecycleOutcome>>().mockResolvedValue(outcome),
		recordExit: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		requestClose: vi
			.fn<(retireProjection: () => Promise<void>) => Promise<{ readonly kind: 'closed' }>>()
			.mockResolvedValue({ kind: 'closed' }),
	};
	const createWindow = vi.fn<DesktopApplicationDependencies['createWindow']>((options) => {
		windowOptions = options;
		return browserWindow;
	});
	const dependencies: DesktopApplicationDependencies = {
		connectHost: vi.fn<DesktopApplicationDependencies['connectHost']>().mockResolvedValue(host),
		createBroker,
		createMessageChannel: () => ({
			port1: {} as MessagePortMain,
			port2: {} as MessagePortMain,
		}),
		createTerminalLifecycle: vi
			.fn<DesktopApplicationDependencies['createTerminalLifecycle']>()
			.mockReturnValue(terminalLifecycle),
		createWindow,
		randomIdentifier: () => identifiers.shift() ?? '88'.repeat(16),
		writeTrace: vi.fn<DesktopApplicationDependencies['writeTrace']>(),
	};
	return {
		brokers,
		createBroker,
		createWindow,
		dependencies,
		host,
		requestControl,
		terminalLifecycle,
		webContents,
		get windowOptions() {
			return windowOptions;
		},
	};
}

function createMetrics(): ProjectedTerminalBrokerMetrics {
	return {
		clipboardCommits: 0,
		commandsAccepted: 0,
		commandsRejected: 0,
		externalTargetsOpened: 0,
		frameBytesForwarded: 0,
		framesForwarded: 0,
		lastSelectionCopyMilliseconds: null,
		lastSelectionExportStartMilliseconds: null,
		rowWindowBytesForwarded: 0,
		rowWindowsForwarded: 0,
		selectionExportBytesForwarded: 0,
		selectionExportChunksForwarded: 0,
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
