import { EventEmitter } from 'node:events';

import {
	AttachmentIdSchema,
	AttachmentCapabilitiesSchema,
	CanonicalUint64TextSchema,
	CorrelationIdSchema,
	CreationIdSchema,
	CURRENT_PROTOCOL_VERSION,
	encodeSelectionExportChunk,
	HostEpochSchema,
	MessageKind,
	PayloadEncoding,
	PROTOCOL_LIMITS,
	SelectionExportStartedSchema,
	TerminalIdSchema,
	TerminalThemeStateSchema,
	type DecodedProtocolEnvelope,
} from '@orchbee/terminal-protocol';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { HostRequestError, type HostConnection } from '../../host';
import { ProjectedTerminalBroker } from '../ProjectedTerminalBroker';

const textEncoder = new TextEncoder();
const theme = TerminalThemeStateSchema.parse({
	operationId: null,
	revision: '0',
	terminal: {
		colorScheme: 'dark',
		defaultBackgroundRgb: 0x1e_1e_1e,
		defaultCursorRgb: null,
		defaultForegroundRgb: 0xd4_d4_d4,
		palette: Array.from({ length: 256 }, (_, index) => index),
	},
});

function eventEnvelope(
	kind: number,
	payloadEncoding: (typeof PayloadEncoding)[keyof typeof PayloadEncoding],
	payload: Uint8Array,
	hostEpoch: ReturnType<typeof HostEpochSchema.parse>,
): DecodedProtocolEnvelope {
	return {
		correlationId: null,
		event: true,
		hostEpoch,
		ignoredOptionalFlags: 0,
		kind,
		knownMessage: true,
		optionalMessage: false,
		payload,
		payloadEncoding,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: false,
	};
}

describe('ProjectedTerminalBroker', () => {
	it('publishes failure and settles locally when the shared Host request transport has ended', async () => {
		let hostClosed: ((reason: Error) => void) | undefined;
		const requestControl = vi.fn<HostConnection['requestControl']>(
			async (_kind, value) => value as never,
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onClosed: vi.fn<HostConnection['onClosed']>((listener) => {
				hostClosed = listener;
				return vi.fn<() => void>();
			}),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
			requestTransportEnded: true,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		let cleanup: Promise<void> | undefined;
		let broker: ProjectedTerminalBroker;
		const onCleanupRequested = vi.fn<(error: unknown) => void>(() => {
			cleanup = broker.dispose();
		});
		broker = new ProjectedTerminalBroker(host, port, bootstrap, undefined, 'desktop', {
			onCleanupRequested,
		});
		broker.start();
		const reason = new Error('Host transport lost');

		hostClosed?.(reason);
		await cleanup;

		expect(postMessage).toHaveBeenCalledWith({
			failure: {
				code: 'host-connection-unavailable',
				message: 'The Terminal Host connection is unavailable.',
				operation: 'protocol',
				retryable: true,
			},
			kind: 'failure',
		});
		expect(onCleanupRequested).toHaveBeenCalledOnce();
		expect(onCleanupRequested).toHaveBeenCalledWith(reason);
		expect(requestControl).not.toHaveBeenCalled();
		expect(port.close).toHaveBeenCalledOnce();
	});

	it('publishes an attachment-scoped resize-lease promotion to its renderer and lifecycle owner', () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: false,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const onResizeLeaseChanged = vi.fn<(ownsResize: boolean) => void>();
		const broker = new ProjectedTerminalBroker(
			host,
			port,
			bootstrap,
			{
				openExternal: vi.fn<(target: string) => Promise<void>>().mockResolvedValue(undefined),
				writeClipboardText: vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined),
			},
			'desktop',
			{
				onCleanupRequested: vi.fn<(error: unknown) => void>(),
				onResizeLeaseChanged,
			},
		);
		broker.start();

		hostListener?.(
			eventEnvelope(
				MessageKind.resizeLeaseChanged,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						attachmentId: bootstrap.attachmentId,
						resizeLease: true,
						terminalId: bootstrap.terminalId,
					}),
				),
				bootstrap.hostEpoch,
			),
		);

		expect(onResizeLeaseChanged).toHaveBeenCalledWith(true);
		expect(postMessage).toHaveBeenCalledWith({
			event: {
				attachmentId: bootstrap.attachmentId,
				resizeLease: true,
				terminalId: bootstrap.terminalId,
			},
			kind: 'resize-lease-changed',
		});
	});

	it('claims resize ownership once through the exact attachment-scoped Host operation', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: false,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const initialCapabilities = AttachmentCapabilitiesSchema.parse({
			attachmentId: bootstrap.attachmentId,
			capabilities: { input: true, read: true, resize: true },
			capabilityRevision: '1',
			resizeLease: false,
			terminalId: bootstrap.terminalId,
		});
		const committedCapabilities = AttachmentCapabilitiesSchema.parse({
			...initialCapabilities,
			resizeLease: true,
		});
		const requestControl = vi.fn<HostConnection['requestControl']>(async (kind) => {
			if (kind !== MessageKind.claimResizeLease) throw new Error('Unexpected Host request.');
			return committedCapabilities as never;
		});
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(
			host,
			port,
			bootstrap,
			undefined,
			'desktop',
			{ onCleanupRequested: vi.fn<(error: unknown) => void>() },
			initialCapabilities,
		);

		await broker.claimResizeLease();
		await broker.claimResizeLease();

		expect(requestControl).toHaveBeenCalledExactlyOnceWith(
			MessageKind.claimResizeLease,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
	});

	it('claims only for accepted meaningful activity from a currently writable attachment', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const requestControl = vi.fn<HostConnection['requestControl']>(async (kind, value) => {
			if (kind === MessageKind.updateAttachmentInputCapability) {
				const request = value as {
					readonly capabilityRevision: ReturnType<typeof CanonicalUint64TextSchema.parse>;
					readonly input: boolean;
				};
				return {
					attachmentId: bootstrap.attachmentId,
					capabilities: { input: request.input, read: true, resize: true },
					capabilityRevision: request.capabilityRevision,
					resizeLease: true,
					terminalId: bootstrap.terminalId,
				} as never;
			}
			return { attachmentId: bootstrap.attachmentId, state: 'none' } as never;
		});
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
			requestInput: vi.fn<HostConnection['requestInput']>().mockResolvedValue({} as never),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const onResizeActivity = vi.fn<() => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, undefined, 'desktop', {
			onCleanupRequested: vi.fn<(error: unknown) => void>(),
			onResizeActivity,
		});
		broker.start();
		const modifiers = {
			alt: false,
			altRight: false,
			capsLock: false,
			control: false,
			controlRight: false,
			meta: false,
			metaRight: false,
			numLock: false,
			shift: false,
			shiftRight: false,
		};
		const authority = {
			attachmentId: bootstrap.attachmentId,
			capabilityRevision: bootstrap.capabilityRevision,
		};
		const send = (data: unknown): void => {
			emitter.emit('message', { data });
		};
		const key = {
			...authority,
			composing: false,
			consumedModifiers: modifiers,
			key: 'KeyA',
			modifiers,
			optionAsAlt: false,
			text: 'a',
			unshiftedCodepoint: 97,
		};
		const mouse = {
			...authority,
			button: 1,
			modifiers,
			pressedButtons: 1,
			x: 10,
			y: 20,
		};
		const selection = {
			attachmentId: bootstrap.attachmentId,
			generation: '1',
			point: { column: 0, row: '0' },
			revision: '1',
		};

		send({ kind: 'focus-input', payload: { ...authority, focused: true } });
		send({ kind: 'focus-input', payload: { ...authority, focused: false } });
		send({ kind: 'key-input', payload: { ...key, action: 'press' } });
		send({ kind: 'key-input', payload: { ...key, action: 'repeat' } });
		send({ kind: 'key-input', payload: { ...key, action: 'release' } });
		send({ kind: 'text-input', payload: { ...authority, text: 'committed' } });
		send({ kind: 'paste-input', payload: { ...authority, bracketed: false, text: 'pasted' } });
		send({ kind: 'mouse-input', payload: { ...mouse, action: 'press' } });
		send({
			kind: 'mouse-input',
			payload: { ...mouse, action: 'motion', pressedButtons: 1 },
		});
		send({
			kind: 'mouse-input',
			payload: { ...mouse, action: 'release', pressedButtons: 0 },
		});
		send({
			kind: 'selection-gesture',
			payload: {
				...selection,
				action: 'start',
				clickCount: 1,
				position: { x: 0, y: 0 },
				selectOutput: false,
			},
		});
		send({
			kind: 'selection-gesture',
			payload: {
				...selection,
				action: 'update',
				geometry: { cellWidth: 10, columns: 80, paddingLeft: 0, screenHeight: 400 },
				position: { x: 10, y: 0 },
				rectangle: false,
			},
		});
		send({ kind: 'selection-gesture', payload: { ...selection, action: 'end' } });
		send({
			kind: 'resize-terminal',
			payload: {
				...authority,
				cellPixelHeight: 20,
				cellPixelWidth: 10,
				columns: 80,
				rows: 24,
			},
		});

		expect(onResizeActivity).toHaveBeenCalledTimes(7);

		await broker.setInputCapability(false);
		onResizeActivity.mockClear();
		send({
			kind: 'selection-gesture',
			payload: {
				...selection,
				action: 'start',
				clickCount: 1,
				position: { x: 0, y: 0 },
				selectOutput: false,
			},
		});

		expect(onResizeActivity).not.toHaveBeenCalled();
		await expect(broker.claimResizeLease()).rejects.toMatchObject({
			code: 'resize-capability-unavailable',
		});
		expect(
			requestControl.mock.calls.filter(([kind]) => kind === MessageKind.claimResizeLease),
		).toEqual([]);
	});

	it('serializes Host resizes and replaces queued measurements with the newest geometry', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const pendingResizes: Array<{
			reject: (error: Error) => void;
			resolve: (value: unknown) => void;
		}> = [];
		const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>((kind) => {
			if (kind !== MessageKind.resizeTerminal) {
				throw new Error(`Unexpected Host request ${String(kind)}`);
			}
			return new Promise((resolve, reject) => {
				pendingResizes.push({ reject, resolve });
			});
		});
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();
		const sendResize = (columns: number): void => {
			emitter.emit('message', {
				data: {
					kind: 'resize-terminal',
					payload: {
						attachmentId: bootstrap.attachmentId,
						cellPixelHeight: 20,
						cellPixelWidth: 10,
						columns,
						rows: 40,
					},
				},
			});
		};
		const acceptResize = (columns: number, revision: string): void => {
			pendingResizes.at(-1)?.resolve({
				attachmentId: bootstrap.attachmentId,
				cellPixelHeight: 20,
				cellPixelWidth: 10,
				columns,
				revision,
				rows: 40,
			});
		};

		sendResize(100);
		sendResize(101);
		sendResize(102);

		expect(requestControl).toHaveBeenCalledTimes(1);
		expect(requestControl).toHaveBeenLastCalledWith(
			MessageKind.resizeTerminal,
			expect.objectContaining({ columns: 100 }),
			expect.anything(),
		);

		acceptResize(100, '1');
		await vi.waitFor(() => expect(requestControl).toHaveBeenCalledTimes(2));
		expect(requestControl).toHaveBeenLastCalledWith(
			MessageKind.resizeTerminal,
			expect.objectContaining({ columns: 102 }),
			expect.anything(),
		);

		sendResize(103);
		sendResize(104);
		expect(requestControl).toHaveBeenCalledTimes(2);
		pendingResizes.at(-1)?.reject(
			new HostRequestError({
				code: 'resize-failed',
				message: 'PTY resize is temporarily unavailable.',
				retryable: true,
			}),
		);

		await vi.waitFor(() => expect(requestControl).toHaveBeenCalledTimes(3));
		expect(requestControl).toHaveBeenLastCalledWith(
			MessageKind.resizeTerminal,
			expect.objectContaining({ columns: 104 }),
			expect.anything(),
		);
		expect(postMessage).toHaveBeenCalledWith({
			failure: {
				code: 'resize-failed',
				message: 'PTY resize is temporarily unavailable.',
				operation: 'resize',
				retryable: true,
			},
			kind: 'failure',
		});
		expect(broker.metrics.commandsAccepted).toBe(5);
		acceptResize(104, '3');
	});

	it('drops a queued resize when attachment cleanup begins', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		let hostClosed: ((reason: Error) => void) | undefined;
		let resolveResize: ((value: unknown) => void) | undefined;
		const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(
			() =>
				new Promise((resolve) => {
					resolveResize = resolve;
				}),
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onClosed: vi.fn<HostConnection['onClosed']>((listener) => {
				hostClosed = listener;
				return vi.fn<() => void>();
			}),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const onCleanupRequested = vi.fn<(error: unknown) => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, undefined, 'desktop', {
			onCleanupRequested,
		});
		broker.start();
		const sendResize = (columns: number): void => {
			emitter.emit('message', {
				data: {
					kind: 'resize-terminal',
					payload: {
						attachmentId: bootstrap.attachmentId,
						cellPixelHeight: 20,
						cellPixelWidth: 10,
						columns,
						rows: 40,
					},
				},
			});
		};

		sendResize(100);
		sendResize(120);
		expect(requestControl).toHaveBeenCalledOnce();

		const reason = new Error('Host closed during resize.');
		hostClosed?.(reason);
		resolveResize?.({
			attachmentId: bootstrap.attachmentId,
			cellPixelHeight: 20,
			cellPixelWidth: 10,
			columns: 100,
			revision: '1',
			rows: 40,
		});
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(requestControl).toHaveBeenCalledOnce();
		expect(onCleanupRequested).toHaveBeenCalledExactlyOnceWith(reason);
	});

	it('publishes live input capability only after an exact Host revision commit', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: false,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const initialCapabilities = AttachmentCapabilitiesSchema.parse({
			attachmentId: bootstrap.attachmentId,
			capabilities: { input: true, read: true, resize: true },
			capabilityRevision: '1',
			resizeLease: false,
			terminalId: bootstrap.terminalId,
		});
		const requestControl = vi.fn<HostConnection['requestControl']>(async (kind, value) => {
			if (kind === MessageKind.detachTerminal) return value as never;
			if (kind !== MessageKind.updateAttachmentInputCapability) {
				throw new Error('Unexpected Host request.');
			}
			const request = value as {
				readonly capabilityRevision: ReturnType<typeof CanonicalUint64TextSchema.parse>;
				readonly input: boolean;
			};
			return {
				...initialCapabilities,
				capabilities: { ...initialCapabilities.capabilities, input: request.input },
				capabilityRevision: request.capabilityRevision,
			} as never;
		});
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(
			host,
			port,
			bootstrap,
			undefined,
			'desktop',
			{ onCleanupRequested: vi.fn<(error: unknown) => void>() },
			initialCapabilities,
		);
		broker.start();

		await broker.setInputCapability(false);
		await broker.setInputCapability(false);
		await broker.setInputCapability(true);

		expect(
			requestControl.mock.calls.filter(
				([kind]) => kind === MessageKind.updateAttachmentInputCapability,
			),
		).toEqual([
			[
				MessageKind.updateAttachmentInputCapability,
				{
					attachmentId: bootstrap.attachmentId,
					capabilityRevision: '2',
					input: false,
				},
				expect.anything(),
			],
			[
				MessageKind.updateAttachmentInputCapability,
				{
					attachmentId: bootstrap.attachmentId,
					capabilityRevision: '3',
					input: true,
				},
				expect.anything(),
			],
		]);
		expect(postMessage.mock.calls.map(([message]) => message)).toEqual([
			{
				capabilities: {
					...initialCapabilities,
					capabilities: { ...initialCapabilities.capabilities, input: false },
					capabilityRevision: '2',
				},
				kind: 'attachment-input-capability-changed',
			},
			{
				capabilities: {
					...initialCapabilities,
					capabilities: { ...initialCapabilities.capabilities, input: true },
					capabilityRevision: '3',
				},
				kind: 'attachment-input-capability-changed',
			},
		]);
	});

	it('fails closed without renderer publication when the Host commits a different revision', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: false,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const initialCapabilities = AttachmentCapabilitiesSchema.parse({
			attachmentId: bootstrap.attachmentId,
			capabilities: { input: true, read: true, resize: true },
			capabilityRevision: '1',
			resizeLease: false,
			terminalId: bootstrap.terminalId,
		});
		const requestControl = vi.fn<HostConnection['requestControl']>(async (kind) => {
			if (kind !== MessageKind.updateAttachmentInputCapability) {
				throw new Error('Unexpected Host request.');
			}
			return {
				...initialCapabilities,
				capabilities: { ...initialCapabilities.capabilities, input: false },
				capabilityRevision: CanonicalUint64TextSchema.parse('3'),
			} as never;
		});
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const onCleanupRequested = vi.fn<(error: unknown) => void>();
		const broker = new ProjectedTerminalBroker(
			host,
			port,
			bootstrap,
			undefined,
			'desktop',
			{ onCleanupRequested },
			initialCapabilities,
		);

		await expect(broker.setInputCapability(false)).rejects.toMatchObject({
			code: 'invalid-attachment-capability-response',
		});
		expect(onCleanupRequested).toHaveBeenCalledOnce();
		expect(postMessage).not.toHaveBeenCalled();
		await expect(broker.setInputCapability(false)).rejects.toMatchObject({
			code: 'attachment-unavailable',
		});
	});

	it('copies exact frame bytes once and never interprets terminal cells in Main', () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		let controlListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>((listener) => {
				controlListener = listener;
				return vi.fn<() => void>();
			}),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();
		const backing = new Uint8Array([99, 1, 2, 3, 88]);
		const payload = backing.subarray(1, 4);
		hostListener?.({
			correlationId: null,
			event: true,
			hostEpoch: bootstrap.hostEpoch,
			ignoredOptionalFlags: 0,
			kind: MessageKind.snapshotFrame,
			knownMessage: true,
			optionalMessage: false,
			payload,
			payloadEncoding: PayloadEncoding.terminalFrame,
			protocol: CURRENT_PROTOCOL_VERSION,
			response: false,
		});

		const posted = postMessage.mock.calls[0]?.[0] as { frame: unknown; kind: string };
		expect(posted.kind).toBe('frame');
		expect(posted.frame).toBeInstanceOf(ArrayBuffer);
		if (!(posted.frame instanceof ArrayBuffer)) {
			throw new Error('Expected an exact projected frame ArrayBuffer.');
		}
		expect(posted.frame.byteLength).toBe(payload.byteLength);
		expect(new Uint8Array(posted.frame)).toEqual(new Uint8Array([1, 2, 3]));
		const clonedFrame = structuredClone(posted.frame);
		expect(clonedFrame.byteLength).toBe(payload.byteLength);
		expect(broker.metrics).toMatchObject({ frameBytesForwarded: 3, framesForwarded: 1 });
		hostListener?.(
			eventEnvelope(
				MessageKind.terminalBell,
				PayloadEncoding.controlJson,
				textEncoder.encode(JSON.stringify({ attachmentId: bootstrap.attachmentId, sequence: '4' })),
				bootstrap.hostEpoch,
			),
		);
		expect(postMessage.mock.calls[1]?.[0]).toEqual({
			event: { attachmentId: bootstrap.attachmentId, sequence: '4' },
			kind: 'bell',
		});
		hostListener?.(
			eventEnvelope(
				MessageKind.terminalClipboardWriteRequest,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						attachmentId: bootstrap.attachmentId,
						sequence: '5',
						text: 'terminal text',
					}),
				),
				bootstrap.hostEpoch,
			),
		);
		expect(postMessage.mock.calls[2]?.[0]).toEqual({
			event: {
				attachmentId: bootstrap.attachmentId,
				sequence: '5',
				text: 'terminal text',
			},
			kind: 'clipboard-write-request',
		});

		controlListener?.(
			eventEnvelope(
				MessageKind.terminalExited,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						exit: { code: 0, kind: 'exit-code' },
						finalRevision: '1',
						identity: {
							creationId: '55'.repeat(16),
							hostEpoch: bootstrap.hostEpoch,
							terminalId: '66'.repeat(16),
						},
					}),
				),
				bootstrap.hostEpoch,
			),
		);
		const exit = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: '1',
			identity: {
				creationId: bootstrap.creationId,
				hostEpoch: bootstrap.hostEpoch,
				terminalId: bootstrap.terminalId,
			},
		};
		const validExitEnvelope = eventEnvelope(
			MessageKind.terminalExited,
			PayloadEncoding.controlJson,
			textEncoder.encode(JSON.stringify(exit)),
			bootstrap.hostEpoch,
		);
		controlListener?.(validExitEnvelope);
		controlListener?.(validExitEnvelope);
		expect(postMessage).toHaveBeenCalledTimes(4);
		expect(postMessage.mock.calls[3]?.[0]).toEqual({
			event: exit,
			kind: 'terminal-exited',
		});
	});

	it('commits only the exact pending clipboard request once after a positive renderer decision', async () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const writeClipboardText = vi
			.fn<(text: string) => Promise<void>>()
			.mockResolvedValue(undefined);
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, {
			openExternal: vi.fn<(target: string) => Promise<void>>().mockResolvedValue(undefined),
			writeClipboardText,
		});
		broker.start();
		const emitClipboardRequest = (sequence: string, text: string): void => {
			hostListener?.(
				eventEnvelope(
					MessageKind.terminalClipboardWriteRequest,
					PayloadEncoding.controlJson,
					textEncoder.encode(
						JSON.stringify({ attachmentId: bootstrap.attachmentId, sequence, text }),
					),
					bootstrap.hostEpoch,
				),
			);
		};
		const sendDecision = (sequence: string, text: string, approved: boolean): void => {
			emitter.emit('message', {
				data: {
					kind: 'clipboard-write-decision',
					payload: { approved, attachmentId: bootstrap.attachmentId, sequence, text },
				},
			});
		};

		emitClipboardRequest('5', 'terminal text');
		expect(postMessage).toHaveBeenCalledWith({
			event: { attachmentId: bootstrap.attachmentId, sequence: '5', text: 'terminal text' },
			kind: 'clipboard-write-request',
		});
		const forwardedCount = postMessage.mock.calls.length;
		emitClipboardRequest('6', 'bounded while a decision is pending');
		expect(postMessage).toHaveBeenCalledTimes(forwardedCount);

		sendDecision('5', 'tampered text', true);
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: expect.objectContaining({ code: 'clipboard-decision-mismatch' }),
				kind: 'failure',
			}),
		);
		expect(writeClipboardText).not.toHaveBeenCalled();

		sendDecision('5', 'terminal text', true);
		await vi.waitFor(() =>
			expect(writeClipboardText).toHaveBeenCalledExactlyOnceWith('terminal text'),
		);
		expect(broker.metrics.clipboardCommits).toBe(1);
		sendDecision('5', 'terminal text', true);
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: expect.objectContaining({ code: 'clipboard-decision-mismatch' }),
				kind: 'failure',
			}),
		);
		expect(writeClipboardText).toHaveBeenCalledOnce();

		emitClipboardRequest('6', 'declined terminal text');
		sendDecision('6', 'declined terminal text', false);
		await vi.waitFor(() => expect(broker.metrics.commandsAccepted).toBe(4));
		expect(writeClipboardText).toHaveBeenCalledOnce();
		expect(broker.metrics.clipboardCommits).toBe(1);
	});

	it('consumes an approved client-target clipboard decision without duplicating its local effect', async () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const writeClipboardText = vi
			.fn<(text: string) => Promise<void>>()
			.mockResolvedValue(undefined);
		const broker = new ProjectedTerminalBroker(
			host,
			port,
			bootstrap,
			{
				openExternal: vi.fn<(target: string) => Promise<void>>().mockResolvedValue(undefined),
				writeClipboardText,
			},
			'client',
		);
		broker.start();
		hostListener?.(
			eventEnvelope(
				MessageKind.terminalClipboardWriteRequest,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						attachmentId: bootstrap.attachmentId,
						sequence: '1',
						text: 'client-owned text',
					}),
				),
				bootstrap.hostEpoch,
			),
		);
		emitter.emit('message', {
			data: {
				kind: 'clipboard-write-decision',
				payload: {
					approved: true,
					attachmentId: bootstrap.attachmentId,
					sequence: '1',
					text: 'client-owned text',
				},
			},
		});

		await vi.waitFor(() => expect(broker.metrics.commandsAccepted).toBe(1));
		expect(writeClipboardText).not.toHaveBeenCalled();
		expect(broker.metrics.clipboardCommits).toBe(0);
	});

	it('absorbs a renderer input burst beyond the active Host request window and drains it in arrival order', async () => {
		const releases: Array<() => void> = [];
		const requestInput = vi.fn<HostConnection['requestInput']>(
			() =>
				new Promise<DecodedProtocolEnvelope>((resolve) => {
					releases.push(() => resolve({} as DecodedProtocolEnvelope));
				}),
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestInput,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const onCleanupRequested = vi.fn<(error: unknown) => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, undefined, 'desktop', {
			onCleanupRequested,
		});
		broker.start();

		for (let index = 0; index < 96; index += 1) {
			emitter.emit('message', {
				data: {
					kind: 'text-input',
					payload: {
						attachmentId: bootstrap.attachmentId,
						capabilityRevision: bootstrap.capabilityRevision,
						text: String(index),
					},
				},
			});
		}

		expect(requestInput).toHaveBeenCalledTimes(16);
		expect(broker.metrics).toMatchObject({ commandsAccepted: 96, commandsRejected: 0 });
		expect(postMessage).not.toHaveBeenCalledWith({
			failure: expect.objectContaining({ code: 'renderer-backpressure' }),
			kind: 'failure',
		});
		expect(onCleanupRequested).not.toHaveBeenCalled();

		releases[7]?.();
		await vi.waitFor(() => expect(requestInput).toHaveBeenCalledTimes(17));
		expect(requestInput.mock.calls[16]?.[1]).toMatchObject({ text: '16' });
		expect(onCleanupRequested).not.toHaveBeenCalled();
	});

	it('retains the total active request limit while draining queued control commands in arrival order', async () => {
		const releases: Array<() => void> = [];
		const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(
			() =>
				new Promise<DecodedProtocolEnvelope>((resolve) => {
					releases.push(() => resolve({} as DecodedProtocolEnvelope));
				}),
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();

		for (let index = 0; index < 96; index += 1) {
			emitter.emit('message', {
				data: {
					kind: 'frame-applied',
					payload: {
						attachmentId: bootstrap.attachmentId,
						generation: '1',
						revision: String(index),
						sequence: String(index + 1),
					},
				},
			});
		}

		expect(requestControl).toHaveBeenCalledTimes(64);
		expect(broker.metrics).toMatchObject({ commandsAccepted: 96, commandsRejected: 0 });
		releases[7]?.();
		await vi.waitFor(() => expect(requestControl).toHaveBeenCalledTimes(65));
		expect(requestControl.mock.calls[64]?.[1]).toMatchObject({ sequence: '65' });
	});

	it('retires only the saturated attachment when the pending renderer window is full', () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const requestInput = vi.fn<HostConnection['requestInput']>(
			() => new Promise<never>(() => undefined),
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestInput,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const onCleanupRequested = vi.fn<(error: unknown) => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, undefined, 'desktop', {
			onCleanupRequested,
		});
		broker.start();
		for (let index = 0; index < 16; index += 1) {
			emitter.emit('message', {
				data: {
					kind: 'text-input',
					payload: {
						attachmentId: bootstrap.attachmentId,
						capabilityRevision: bootstrap.capabilityRevision,
						text: String(index),
					},
				},
			});
		}
		expect(requestInput).toHaveBeenCalledTimes(16);
		for (let index = 16; index < 1_040; index += 1) {
			emitter.emit('message', {
				data: {
					kind: 'text-input',
					payload: {
						attachmentId: bootstrap.attachmentId,
						capabilityRevision: bootstrap.capabilityRevision,
						text: String(index),
					},
				},
			});
		}
		const emitClipboardRequest = (sequence: string): void => {
			hostListener?.(
				eventEnvelope(
					MessageKind.terminalClipboardWriteRequest,
					PayloadEncoding.controlJson,
					textEncoder.encode(
						JSON.stringify({
							attachmentId: bootstrap.attachmentId,
							sequence,
							text: `request ${sequence}`,
						}),
					),
					bootstrap.hostEpoch,
				),
			);
		};
		emitClipboardRequest('1');
		emitter.emit('message', {
			data: {
				kind: 'clipboard-write-decision',
				payload: {
					approved: false,
					attachmentId: bootstrap.attachmentId,
					sequence: '1',
					text: 'request 1',
				},
			},
		});
		expect(postMessage).toHaveBeenCalledWith({
			failure: expect.objectContaining({
				code: 'renderer-backpressure',
				operation: 'clipboard',
				retryable: true,
			}),
			kind: 'failure',
		});
		expect(onCleanupRequested).toHaveBeenCalledOnce();

		const postedCount = postMessage.mock.calls.length;
		emitClipboardRequest('2');
		expect(postMessage).toHaveBeenCalledTimes(postedCount);
		expect(broker.metrics.commandsRejected).toBe(1);
	});

	it('bounds pending renderer commands by serialized bytes independently of entry count', () => {
		const requestInput = vi.fn<HostConnection['requestInput']>(
			() => new Promise<never>(() => undefined),
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestInput,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const onCleanupRequested = vi.fn<(error: unknown) => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, undefined, 'desktop', {
			onCleanupRequested,
		});
		broker.start();
		for (let index = 0; index < 16; index += 1) {
			emitter.emit('message', {
				data: {
					kind: 'text-input',
					payload: {
						attachmentId: bootstrap.attachmentId,
						capabilityRevision: bootstrap.capabilityRevision,
						text: String(index),
					},
				},
			});
		}
		const largeText = 'x'.repeat(PROTOCOL_LIMITS.maxInputBytes);
		for (let index = 0; index < 4; index += 1) {
			emitter.emit('message', {
				data: {
					kind: 'text-input',
					payload: {
						attachmentId: bootstrap.attachmentId,
						capabilityRevision: bootstrap.capabilityRevision,
						text: largeText,
					},
				},
			});
		}

		expect(requestInput).toHaveBeenCalledTimes(16);
		expect(broker.metrics).toMatchObject({ commandsAccepted: 19, commandsRejected: 1 });
		expect(postMessage).toHaveBeenCalledWith({
			failure: expect.objectContaining({
				code: 'renderer-backpressure',
				operation: 'input',
				retryable: true,
			}),
			kind: 'failure',
		});
		expect(onCleanupRequested).toHaveBeenCalledOnce();
	});

	it('retires only the affected attachment when Desktop-to-Host admission rejects an ACK', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const failure = new HostRequestError({
			code: 'host-transport-backpressure',
			message: 'The bounded Desktop-to-Host request transport is saturated.',
			retryable: true,
		});
		const requestControl = vi.fn<HostConnection['requestControl']>().mockRejectedValue(failure);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const onCleanupRequested = vi.fn<(error: unknown) => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, undefined, 'desktop', {
			onCleanupRequested,
		});
		broker.start();

		emitter.emit('message', {
			data: {
				kind: 'frame-applied',
				payload: {
					attachmentId: bootstrap.attachmentId,
					generation: '1',
					revision: '1',
					sequence: '1',
				},
			},
		});

		await vi.waitFor(() => {
			expect(postMessage).toHaveBeenCalledWith({
				failure: {
					code: 'host-transport-backpressure',
					message: 'The bounded Desktop-to-Host request transport is saturated.',
					operation: 'protocol',
					retryable: true,
				},
				kind: 'failure',
			});
		});
		expect(onCleanupRequested).toHaveBeenCalledOnce();
		expect(onCleanupRequested).toHaveBeenCalledWith(failure);
	});

	it('rejects commands for another attachment before calling the Host', () => {
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestInput: vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(),
			requestControl: vi
				.fn<(...arguments_: unknown[]) => Promise<unknown>>()
				.mockResolvedValue(undefined),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, {
			attachmentId: AttachmentIdSchema.parse('55'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('66'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('77'.repeat(16)),
			terminalId: TerminalIdSchema.parse('88'.repeat(16)),
			terminalOutcome: null,
			theme,
		});
		broker.start();
		emitter.emit('message', {
			data: {
				kind: 'text-input',
				payload: {
					attachmentId: AttachmentIdSchema.parse('99'.repeat(16)),
					capabilityRevision: CanonicalUint64TextSchema.parse('1'),
					text: 'blocked',
				},
			},
		});

		expect(host.requestInput).not.toHaveBeenCalled();
		expect(postMessage).toHaveBeenCalledWith({
			failure: expect.objectContaining({ code: 'renderer-capability-rejected' }),
			kind: 'failure',
		});
		expect(broker.metrics.commandsRejected).toBe(1);
	});

	it('forwards runtime theme apply and reconciliation with complete lifecycle state', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const operationId = '55'.repeat(16);
		const reconcileOperationId = '66'.repeat(16);
		const terminal = {
			...theme.terminal,
			colorScheme: 'light' as const,
			defaultBackgroundRgb: 0xfc_fc_fc,
			defaultForegroundRgb: 0x14_14_14,
		};
		const current = TerminalThemeStateSchema.parse({
			operationId,
			revision: '1',
			terminal,
		});
		const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(async (kind) => {
			if (kind === MessageKind.applyTerminalTheme) {
				return {
					current,
					operationId,
					terminalId: bootstrap.terminalId,
					transition: 'applied',
				};
			}
			if (kind === MessageKind.listTerminals) {
				return {
					inventoryGeneration: '1',
					nextCursor: null,
					terminals: [
						{
							identity: {
								creationId: bootstrap.creationId,
								hostEpoch: bootstrap.hostEpoch,
								terminalId: bootstrap.terminalId,
							},
							theme: current,
						},
					],
				};
			}
			throw new Error(`Unexpected Host request ${String(kind)}`);
		});
		const host = {
			hostEpoch: bootstrap.hostEpoch,
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const onThemeChanged = vi.fn<(nextTheme: typeof current) => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, undefined, 'desktop', {
			onCleanupRequested: vi.fn<(error: unknown) => void>(),
			onThemeChanged,
		});
		broker.start();

		emitter.emit('message', {
			data: {
				kind: 'apply-terminal-theme',
				payload: {
					attachmentId: bootstrap.attachmentId,
					expectedThemeRevision: '0',
					operationId,
					terminal,
					terminalId: bootstrap.terminalId,
				},
			},
		});
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				event: expect.objectContaining({ current, operationId }),
				kind: 'terminal-theme-result',
			}),
		);
		expect(onThemeChanged).toHaveBeenLastCalledWith(current);

		emitter.emit('message', {
			data: {
				kind: 'reconcile-terminal-theme',
				payload: {
					attachmentId: bootstrap.attachmentId,
					operationId: reconcileOperationId,
					terminalId: bootstrap.terminalId,
				},
			},
		});
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				current,
				kind: 'terminal-theme-reconciled',
				operationId: reconcileOperationId,
				terminalId: bootstrap.terminalId,
			}),
		);
		expect(onThemeChanged).toHaveBeenCalledTimes(2);
	});

	it('reports an unknown Host theme outcome without a generic attachment failure', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const operationId = '55'.repeat(16);
		const requestControl = vi.fn<HostConnection['requestControl']>().mockRejectedValue(
			new HostRequestError({
				code: 'host-request-outcome-unknown',
				message: 'The Host may have committed the theme operation.',
				retryable: true,
			}),
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();

		emitter.emit('message', {
			data: {
				kind: 'apply-terminal-theme',
				payload: {
					attachmentId: bootstrap.attachmentId,
					expectedThemeRevision: '0',
					operationId,
					terminal: theme.terminal,
					terminalId: bootstrap.terminalId,
				},
			},
		});
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: {
					code: 'host-request-outcome-unknown',
					outcome: 'unknown',
					retryable: true,
				},
				kind: 'terminal-theme-failed',
				operationId,
			}),
		);
		expect(postMessage).not.toHaveBeenCalledWith(expect.objectContaining({ kind: 'failure' }));
	});

	it('forwards clear and reset as typed Host controls and rejects a mismatched acknowledgement', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		let resetCount = 0;
		const requestControl = vi.fn<(kind: number) => Promise<unknown>>(async (kind) => {
			const operation = kind === MessageKind.clearTerminal ? 'clear' : 'reset';
			if (kind === MessageKind.resetTerminal) {
				resetCount += 1;
				if (resetCount === 3) {
					throw new HostRequestError(
						{
							code: 'terminal-control-backpressure',
							message: 'The Host control queue is full.',
							retryable: true,
						},
						CorrelationIdSchema.parse('55'.repeat(16)),
					);
				}
			}
			return {
				attachmentId: bootstrap.attachmentId,
				generation: String(resetCount + 1),
				operation: resetCount > 1 ? 'clear' : operation,
				revision: String(resetCount + 1),
				terminalId: bootstrap.terminalId,
			};
		});
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();

		for (const kind of ['clear-terminal', 'reset-terminal'] as const) {
			emitter.emit('message', {
				data: { kind, payload: { attachmentId: bootstrap.attachmentId } },
			});
		}
		await vi.waitFor(() => expect(requestControl).toHaveBeenCalledTimes(2));
		expect(requestControl).toHaveBeenNthCalledWith(
			1,
			MessageKind.clearTerminal,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
		expect(requestControl).toHaveBeenNthCalledWith(
			2,
			MessageKind.resetTerminal,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
		expect(postMessage).not.toHaveBeenCalled();

		emitter.emit('message', {
			data: { kind: 'reset-terminal', payload: { attachmentId: bootstrap.attachmentId } },
		});
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: expect.objectContaining({
					code: 'invalid-terminal-control-response',
					operation: 'control',
					retryable: false,
				}),
				kind: 'failure',
			}),
		);

		emitter.emit('message', {
			data: { kind: 'reset-terminal', payload: { attachmentId: bootstrap.attachmentId } },
		});
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: {
					code: 'terminal-control-backpressure',
					correlationId: CorrelationIdSchema.parse('55'.repeat(16)),
					message: 'The Host control queue is full.',
					operation: 'control',
					retryable: true,
				},
				kind: 'failure',
			}),
		);
	});

	it('routes one bounded row response without interpreting its cells in Main', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const responseBacking = new Uint8Array([99, 7, 8, 9, 88]);
		const responsePayload = responseBacking.subarray(1, 4);
		const requestControlEnvelope = vi
			.fn<
				(...arguments_: unknown[]) => Promise<{
					readonly payload: Uint8Array;
					readonly payloadEncoding: typeof PayloadEncoding.rowWindow;
				}>
			>()
			.mockResolvedValue({
				payload: responsePayload,
				payloadEncoding: PayloadEncoding.rowWindow,
			});
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControlEnvelope,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();
		emitter.emit('message', {
			data: {
				kind: 'read-rows',
				payload: {
					attachmentId: bootstrap.attachmentId,
					detail: 'text',
					generation: '1',
					maximumBytes: 1024,
					origin: { kind: 'absolute', row: '0' },
					purpose: 'buffer',
					requestSequence: '1',
					revision: '1',
					rowCount: 1,
				},
			},
		});

		await vi.waitFor(() => {
			expect(postMessage).toHaveBeenCalledWith(
				expect.objectContaining({ kind: 'rows', rows: expect.any(ArrayBuffer) }),
			);
		});
		const posted = postMessage.mock.calls[0]?.[0] as { rows: unknown };
		expect(posted.rows).toBeInstanceOf(ArrayBuffer);
		if (!(posted.rows instanceof ArrayBuffer)) {
			throw new Error('Expected an exact projected row-window ArrayBuffer.');
		}
		expect(posted.rows.byteLength).toBe(responsePayload.byteLength);
		expect(new Uint8Array(posted.rows)).toEqual(new Uint8Array([7, 8, 9]));
		const clonedRows = structuredClone(posted.rows);
		expect(clonedRows.byteLength).toBe(responsePayload.byteLength);
		expect(broker.metrics).toMatchObject({
			rowWindowBytesForwarded: 3,
			rowWindowsForwarded: 1,
		});
	});

	it('preserves the renderer row-request sequence on a typed Host failure', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const correlationId = CorrelationIdSchema.parse('55'.repeat(16));
		const requestControlEnvelope = vi
			.fn<HostConnection['requestControlEnvelope']>()
			.mockRejectedValue(
				new HostRequestError(
					{
						code: 'stale-revision',
						message: 'ReadRows used a stale revision.',
						retryable: true,
					},
					correlationId,
				),
			);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControlEnvelope,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();
		emitter.emit('message', {
			data: {
				kind: 'read-rows',
				payload: {
					attachmentId: bootstrap.attachmentId,
					detail: 'render',
					generation: '1',
					maximumBytes: 1024,
					origin: { delta: 0, kind: 'committed' },
					purpose: 'viewport',
					requestSequence: '7',
					revision: '1',
					rowCount: 1,
				},
			},
		});

		await vi.waitFor(() => {
			expect(postMessage).toHaveBeenCalledWith({
				failure: {
					code: 'stale-revision',
					correlationId,
					message: 'ReadRows used a stale revision.',
					operation: 'history',
					requestSequence: '7',
					retryable: true,
				},
				kind: 'failure',
			});
		});
	});

	it('streams a bounded Host selection directly into Main clipboard exactly once', async () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const requestControl = vi.fn<(kind: number, value: unknown) => Promise<unknown>>(
			async (kind, value) => {
				if (kind === MessageKind.selectionExport) {
					const exportRequest = value as { exportSequence: string };
					return {
						attachmentId: bootstrap.attachmentId,
						byteLength: 8,
						exportSequence: exportRequest.exportSequence,
						format: 'plain',
					};
				}
				if (kind === MessageKind.selectionExportChunkApplied) return value;
				throw new Error(`unexpected Host control kind ${kind}`);
			},
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const writeClipboardText = vi.fn<(text: string) => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, {
			openExternal: vi.fn<(target: string) => Promise<void>>().mockResolvedValue(undefined),
			writeClipboardText,
		});
		broker.start();
		emitter.emit('message', {
			data: {
				kind: 'selection-copy',
				payload: { attachmentId: bootstrap.attachmentId, requestSequence: '7' },
			},
		});
		await vi.waitFor(() => expect(requestControl).toHaveBeenCalled());
		expect(requestControl).toHaveBeenCalledWith(
			MessageKind.selectionExport,
			{
				attachmentId: bootstrap.attachmentId,
				exportSequence: '1',
				format: 'plain',
			},
			SelectionExportStartedSchema,
		);
		const chunk = encodeSelectionExportChunk({
			attachmentId: bootstrap.attachmentId,
			bytes: textEncoder.encode('selected'),
			chunkSequence: 0,
			exportSequence: 1n,
			final: true,
		});
		if (!chunk.ok) throw new Error(chunk.detail);
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportChunk,
				PayloadEncoding.rawBytes,
				chunk.value,
				bootstrap.hostEpoch,
			),
		);
		await vi.waitFor(() => {
			expect(requestControl).toHaveBeenCalledWith(
				MessageKind.selectionExportChunkApplied,
				expect.objectContaining({ chunkSequence: 0, exportSequence: '1' }),
				expect.anything(),
			);
		});
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportFinished,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						attachmentId: bootstrap.attachmentId,
						byteLength: 8,
						exportSequence: '1',
						status: 'completed',
					}),
				),
				bootstrap.hostEpoch,
			),
		);

		await vi.waitFor(() => expect(writeClipboardText).toHaveBeenCalledWith('selected'));
		expect(writeClipboardText).toHaveBeenCalledTimes(1);
		expect(postMessage).toHaveBeenCalledWith({
			byteLength: 8,
			kind: 'selection-copied',
			requestSequence: CanonicalUint64TextSchema.parse('7'),
		});
		expect(postMessage.mock.calls).not.toContainEqual([
			expect.objectContaining({ text: 'selected' }),
		]);
		expect(broker.metrics).toMatchObject({
			clipboardCommits: 1,
			selectionExportBytesForwarded: 8,
			selectionExportChunksForwarded: 1,
		});
		expect(broker.metrics.lastSelectionCopyMilliseconds).toBeGreaterThanOrEqual(0);
		expect(broker.metrics.lastSelectionExportStartMilliseconds).toBeGreaterThanOrEqual(0);
	});

	it('ignores a predecessor cancellation while the next selection export is still starting', async () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		let resolveFirstStart: ((value: unknown) => void) | undefined;
		let resolveSecondStart: ((value: unknown) => void) | undefined;
		let selectionExportRequests = 0;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const firstStarted = {
			attachmentId: bootstrap.attachmentId,
			byteLength: 8,
			exportSequence: '1',
			format: 'plain',
		};
		const secondStarted = {
			attachmentId: bootstrap.attachmentId,
			byteLength: 8,
			exportSequence: '2',
			format: 'plain',
		};
		const cancelledFirstExport = (): void => {
			hostListener?.(
				eventEnvelope(
					MessageKind.selectionExportFinished,
					PayloadEncoding.controlJson,
					textEncoder.encode(
						JSON.stringify({
							attachmentId: bootstrap.attachmentId,
							byteLength: firstStarted.byteLength,
							exportSequence: firstStarted.exportSequence,
							status: 'cancelled',
						}),
					),
					bootstrap.hostEpoch,
				),
			);
		};
		const failureCount = (): number =>
			postMessage.mock.calls.filter(
				([message]) => (message as { kind?: string }).kind === 'failure',
			).length;
		const requestControl = vi.fn<(kind: number, value: unknown) => Promise<unknown>>(
			async (kind, value) => {
				if (kind === MessageKind.selectionExport) {
					selectionExportRequests += 1;
					const exportRequest = value as { exportSequence: string };
					if (selectionExportRequests === 1) {
						if (exportRequest.exportSequence !== firstStarted.exportSequence) {
							throw new Error(`unexpected first export sequence ${exportRequest.exportSequence}`);
						}
						return new Promise((resolve) => {
							resolveFirstStart = resolve;
						});
					}
					if (
						selectionExportRequests !== 2 ||
						exportRequest.exportSequence !== secondStarted.exportSequence
					) {
						throw new Error(`unexpected selection export request ${selectionExportRequests}`);
					}
					// HostConnection can route an event from the same read before the
					// response to this request is resolved.
					cancelledFirstExport();
					return new Promise((resolve) => {
						resolveSecondStart = resolve;
					});
				}
				if (kind === MessageKind.selectionExportChunkApplied) return value;
				throw new Error(`unexpected Host control kind ${kind}`);
			},
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const writeClipboardText = vi.fn<(text: string) => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, {
			openExternal: vi.fn<(target: string) => Promise<void>>().mockResolvedValue(undefined),
			writeClipboardText,
		});
		broker.start();

		emitter.emit('message', {
			data: {
				kind: 'selection-copy',
				payload: { attachmentId: bootstrap.attachmentId, requestSequence: '7' },
			},
		});
		await vi.waitFor(() =>
			expect(requestControl).toHaveBeenCalledWith(
				MessageKind.selectionExport,
				expect.objectContaining({ exportSequence: firstStarted.exportSequence }),
				expect.anything(),
			),
		);
		expect(resolveFirstStart).toBeTypeOf('function');
		resolveFirstStart?.(firstStarted);
		await vi.waitFor(() =>
			expect(broker.metrics.lastSelectionExportStartMilliseconds).toBeGreaterThanOrEqual(0),
		);
		cancelledFirstExport();
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: expect.objectContaining({
					code: 'copy-cancelled',
					operation: 'copy',
					requestSequence: '7',
					retryable: true,
				}),
				kind: 'failure',
			}),
		);
		expect(failureCount()).toBe(1);

		emitter.emit('message', {
			data: {
				kind: 'selection-copy',
				payload: { attachmentId: bootstrap.attachmentId, requestSequence: '8' },
			},
		});
		await vi.waitFor(() => expect(selectionExportRequests).toBe(2));
		await Promise.resolve();
		expect(failureCount()).toBe(1);
		expect(resolveSecondStart).toBeTypeOf('function');
		resolveSecondStart?.(secondStarted);
		const secondChunk = encodeSelectionExportChunk({
			attachmentId: bootstrap.attachmentId,
			bytes: textEncoder.encode('selected'),
			chunkSequence: 0,
			exportSequence: 2n,
			final: true,
		});
		if (!secondChunk.ok) throw new Error(secondChunk.detail);
		await vi.waitFor(() =>
			expect(requestControl).toHaveBeenCalledWith(
				MessageKind.selectionExport,
				expect.objectContaining({ exportSequence: secondStarted.exportSequence }),
				expect.anything(),
			),
		);
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportChunk,
				PayloadEncoding.rawBytes,
				secondChunk.value,
				bootstrap.hostEpoch,
			),
		);
		await vi.waitFor(() =>
			expect(requestControl).toHaveBeenCalledWith(
				MessageKind.selectionExportChunkApplied,
				expect.objectContaining({ exportSequence: secondStarted.exportSequence }),
				expect.anything(),
			),
		);
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportFinished,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						attachmentId: bootstrap.attachmentId,
						byteLength: secondStarted.byteLength,
						exportSequence: secondStarted.exportSequence,
						status: 'completed',
					}),
				),
				bootstrap.hostEpoch,
			),
		);
		await vi.waitFor(() => expect(writeClipboardText).toHaveBeenCalledWith('selected'));
		expect(writeClipboardText).toHaveBeenCalledTimes(1);
		expect(failureCount()).toBe(1);
	});

	it('returns Host selection bytes to a Browser attachment without granting Main effects', async () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const requestControl = vi.fn<(kind: number, value: unknown) => Promise<unknown>>(
			async (kind, value) => {
				if (kind === MessageKind.selectionExport) {
					const exportRequest = value as { exportSequence: string };
					return {
						attachmentId: bootstrap.attachmentId,
						byteLength: 8,
						exportSequence: exportRequest.exportSequence,
						format: 'plain',
					};
				}
				if (kind === MessageKind.selectionExportChunkApplied) return value;
				throw new Error(`unexpected Host control kind ${kind}`);
			},
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const openExternal = vi.fn<(_target: string) => Promise<void>>().mockResolvedValue();
		const writeClipboardText = vi.fn<(text: string) => void>();
		const broker = new ProjectedTerminalBroker(
			host,
			port,
			bootstrap,
			{ openExternal, writeClipboardText },
			'client',
		);
		broker.start();
		emitter.emit('message', {
			data: {
				kind: 'selection-copy',
				payload: { attachmentId: bootstrap.attachmentId, requestSequence: '7' },
			},
		});
		await vi.waitFor(() => expect(requestControl).toHaveBeenCalled());
		const chunk = encodeSelectionExportChunk({
			attachmentId: bootstrap.attachmentId,
			bytes: textEncoder.encode('selected'),
			chunkSequence: 0,
			exportSequence: 1n,
			final: true,
		});
		if (!chunk.ok) throw new Error(chunk.detail);
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportChunk,
				PayloadEncoding.rawBytes,
				chunk.value,
				bootstrap.hostEpoch,
			),
		);
		await vi.waitFor(() =>
			expect(requestControl).toHaveBeenCalledWith(
				MessageKind.selectionExportChunkApplied,
				expect.anything(),
				expect.anything(),
			),
		);
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportFinished,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						attachmentId: bootstrap.attachmentId,
						byteLength: 8,
						exportSequence: '1',
						status: 'completed',
					}),
				),
				bootstrap.hostEpoch,
			),
		);

		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith(
				expect.objectContaining({ kind: 'selection-copy-content', requestSequence: '7' }),
			),
		);
		const copyEvent = postMessage.mock.calls.find(
			(call) => (call[0] as { kind?: string }).kind === 'selection-copy-content',
		)?.[0] as { content: ArrayBuffer } | undefined;
		expect(new TextDecoder().decode(copyEvent?.content)).toBe('selected');
		expect(writeClipboardText).not.toHaveBeenCalled();
		expect(broker.metrics.clipboardCommits).toBe(0);

		emitter.emit('message', {
			data: {
				kind: 'link-open',
				payload: {
					attachmentId: bootstrap.attachmentId,
					requestSequence: '8',
					target: 'https://orchbee.dev',
				},
			},
		});
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: expect.objectContaining({ code: 'link-client-effect-required' }),
				kind: 'failure',
			}),
		);
		expect(openExternal).not.toHaveBeenCalled();
	});

	it('allows only explicit web and mail schemes through the external-link effect', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const openExternal = vi.fn<(_target: string) => Promise<void>>().mockResolvedValue(undefined);
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, {
			openExternal,
			writeClipboardText: vi.fn<(text: string) => void>(),
		});
		broker.start();
		for (const [requestSequence, target] of [
			['1', 'http://orchbee.dev/path'],
			['2', 'https://orchbee.dev/终端'],
			['3', 'mailto:terminal@orchbee.dev'],
			['4', 'file:///tmp/secret'],
			['5', 'ftp://orchbee.dev'],
			['6', 'ssh://orchbee.dev'],
			['7', 'tel:+15551234567'],
		] as const) {
			emitter.emit('message', {
				data: {
					kind: 'link-open',
					payload: { attachmentId: bootstrap.attachmentId, requestSequence, target },
				},
			});
		}

		await vi.waitFor(() => expect(openExternal).toHaveBeenCalledTimes(3));
		expect(openExternal.mock.calls.map(([target]) => target)).toEqual([
			'http://orchbee.dev/path',
			'https://orchbee.dev/%E7%BB%88%E7%AB%AF',
			'mailto:terminal@orchbee.dev',
		]);
		for (const requestSequence of ['1', '2', '3'] as const) {
			await vi.waitFor(() => {
				expect(postMessage).toHaveBeenCalledWith({
					kind: 'link-opened',
					requestSequence: CanonicalUint64TextSchema.parse(requestSequence),
				});
			});
		}
		await vi.waitFor(() => {
			expect(
				postMessage.mock.calls.filter(
					([message]) =>
						(message as { failure?: { code?: string } }).failure?.code === 'link-policy-blocked',
				),
			).toHaveLength(4);
		});
		expect(broker.metrics.externalTargetsOpened).toBe(3);
	});

	it('hands renderer postMessage failure to the cleanup owner before detaching', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const requestControl = vi
			.fn<(...arguments_: unknown[]) => Promise<unknown>>()
			.mockResolvedValue({ attachmentId: bootstrap.attachmentId });
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>().mockImplementationOnce(() => {
			throw new Error('renderer port failed');
		});
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		let broker: ProjectedTerminalBroker;
		const onCleanupRequested = vi.fn<() => void>(() => {
			void broker.dispose().catch(() => undefined);
		});
		broker = new ProjectedTerminalBroker(host, port, bootstrap, undefined, 'desktop', {
			onCleanupRequested,
		});
		broker.start();

		emitter.emit('message', { data: { malformed: true } });

		await vi.waitFor(() => expect(requestControl).toHaveBeenCalledOnce());
		expect(onCleanupRequested).toHaveBeenCalledOnce();
		expect(requestControl).toHaveBeenCalledWith(
			MessageKind.detachTerminal,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
	});

	it('awaits Host detach exactly once before completing attachment retirement', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		let finishDetach: (() => void) | undefined;
		const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(
			() =>
				new Promise((resolve) => {
					finishDetach = () => resolve({ attachmentId: bootstrap.attachmentId });
				}),
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();

		const first = broker.dispose();
		const second = broker.dispose();
		expect(first).toBe(second);
		expect(requestControl).toHaveBeenCalledWith(
			MessageKind.detachTerminal,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
		let completed = false;
		void first.then(() => {
			completed = true;
		});
		await Promise.resolve();
		expect(completed).toBe(false);
		finishDetach?.();
		await first;
		expect(completed).toBe(true);
		expect(requestControl).toHaveBeenCalledOnce();
	});

	it('surfaces a failed detach and retries the same AttachmentId', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const requestControl = vi
			.fn<(...arguments_: unknown[]) => Promise<unknown>>()
			.mockRejectedValueOnce(new Error('detach unavailable'))
			.mockResolvedValue({ attachmentId: bootstrap.attachmentId });
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();

		await expect(broker.dispose()).rejects.toThrow('detach unavailable');
		await expect(broker.dispose()).resolves.toBeUndefined();
		expect(requestControl).toHaveBeenCalledTimes(2);
		expect(requestControl).toHaveBeenNthCalledWith(
			1,
			MessageKind.detachTerminal,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
	});

	it('treats a mismatched detach ACK as a retryable cleanup failure', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
			theme,
		};
		const requestControl = vi
			.fn<(...arguments_: unknown[]) => Promise<unknown>>()
			.mockResolvedValueOnce({ attachmentId: AttachmentIdSchema.parse('ff'.repeat(16)) })
			.mockResolvedValue({ attachmentId: bootstrap.attachmentId });
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();

		await expect(broker.dispose()).rejects.toMatchObject({ code: 'attachment-ack-mismatch' });
		await expect(broker.dispose()).resolves.toBeUndefined();
		expect(requestControl).toHaveBeenCalledTimes(2);
	});
});
