import { EventEmitter } from 'node:events';

import {
	AttachmentIdSchema,
	CreationIdSchema,
	CURRENT_PROTOCOL_VERSION,
	HostEpochSchema,
	PROTOCOL_LIMITS,
	TerminalIdSchema,
} from '@orchbee/terminal-protocol';
import type { BrowserWindow, BrowserWindowConstructorOptions, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalBrokerMetrics } from '../../broker';
import { DesktopApplication, type DesktopApplicationDependencies } from '../DesktopApplication';
import type { HostConnection } from '../../host';
import { resolveDesktopRuntimeConfiguration } from '../resolveDesktopRuntimeConfiguration';

describe('DesktopApplication', () => {
	it('creates one sandboxed window and transfers only its scoped attachment port', async () => {
		const identity = {
			creationId: CreationIdSchema.parse('11'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('22'.repeat(16)),
			terminalId: TerminalIdSchema.parse('33'.repeat(16)),
		};
		const requestControl = vi
			.fn<(...arguments_: unknown[]) => Promise<unknown>>()
			.mockResolvedValueOnce(identity)
			.mockResolvedValueOnce({
				attachmentId: AttachmentIdSchema.parse('55'.repeat(16)),
				capabilities: { input: true, read: true, resize: true },
				resizeLease: true,
				terminalId: identity.terminalId,
			});
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
				protocol: CURRENT_PROTOCOL_VERSION,
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
				webContents.emit('did-finish-load');
			}),
			loadURL: vi.fn<BrowserWindow['loadURL']>().mockResolvedValue(undefined),
			show: vi.fn<BrowserWindow['show']>(),
			webContents,
		}) as unknown as BrowserWindow;
		let windowOptions: BrowserWindowConstructorOptions | undefined;
		const broker = {
			dispose: vi.fn<() => void>(),
			metrics: {
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
			} satisfies ProjectedTerminalBrokerMetrics,
			start: vi.fn<() => void>(),
		};
		const port1 = {} as MessagePortMain;
		const port2 = {} as MessagePortMain;
		const identifiers = ['44'.repeat(16), '55'.repeat(16)];
		const dependencies: DesktopApplicationDependencies = {
			connectHost: vi.fn<DesktopApplicationDependencies['connectHost']>().mockResolvedValue(host),
			createBroker: vi.fn<DesktopApplicationDependencies['createBroker']>(() => broker),
			createMessageChannel: () => ({ port1, port2 }),
			createWindow: vi.fn<DesktopApplicationDependencies['createWindow']>((options) => {
				windowOptions = options;
				return browserWindow;
			}),
			randomIdentifier: () => identifiers.shift() ?? '66'.repeat(16),
			writeTrace: vi.fn<DesktopApplicationDependencies['writeTrace']>(),
		};
		const runtime = resolveDesktopRuntimeConfiguration({
			applicationPath: '/repo/apps/desktop',
			environment: { SHELL: '/bin/sh' },
			homeDirectory: '/users/orchbee',
			platform: 'linux',
			userDataPath: '/users/orchbee/.config/orchbee',
		});
		const application = new DesktopApplication(
			{
				preloadPath: '/repo/apps/desktop/dist/preload/preload.cjs',
				rendererFilePath: '/repo/apps/desktop/dist/renderer/index.html',
				runtime,
				traceCopyPath: false,
			},
			dependencies,
		);

		await application.start();

		expect(windowOptions?.webPreferences).toMatchObject({
			contextIsolation: true,
			nodeIntegration: false,
			sandbox: true,
			webSecurity: true,
		});
		expect(broker.start).toHaveBeenCalledOnce();
		expect(webContents.postMessage).toHaveBeenCalledWith(
			'orchbee.desktop.projected-attachment-port',
			expect.objectContaining({ attachmentId: '55'.repeat(16) }),
			[port1],
		);
		expect(requestControl).toHaveBeenCalledTimes(2);
		application.shutdown();
		expect(host.close).toHaveBeenCalledOnce();
	});
});
