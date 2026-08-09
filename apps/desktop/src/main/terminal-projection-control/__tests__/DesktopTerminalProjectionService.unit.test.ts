import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	AttachmentAcknowledgedSchema,
	HostEpochSchema,
	MessageKind,
	TerminalIdSchema,
	type AttachmentId,
	type TerminalExited,
	type TerminalFailed,
} from '@orchbee/terminal-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalBrokerMetrics } from '../../broker';
import type { HostConnection } from '../../host';
import {
	DesktopTerminalProjectionService,
	type DesktopTerminalProjectionServiceDependencies,
} from '../DesktopTerminalProjectionService';

const EMPTY_METRICS: ProjectedTerminalBrokerMetrics = {
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

function createHarness(readOnly = false) {
	const identity = {
		creationId: CreationIdSchema.parse('22'.repeat(16)),
		hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
		terminalId: TerminalIdSchema.parse('44'.repeat(16)),
	};
	let target: {
		identity: typeof identity;
		readOnly: boolean;
		terminalOutcome:
			| { event: TerminalExited; kind: 'exited' }
			| { event: TerminalFailed; kind: 'failed' }
			| null;
	} = {
		identity,
		readOnly,
		terminalOutcome: null,
	};
	const requestControl = vi.fn<HostConnection['requestControl']>(async (kind, value) => {
		if (kind !== MessageKind.attachTerminal) return value as never;
		const request = value as {
			readonly attachmentId: AttachmentId;
			readonly capabilities: {
				readonly input: boolean;
				readonly read: boolean;
				readonly resize: boolean;
			};
			readonly requestResizeLease: boolean;
		};
		return {
			attachmentId: request.attachmentId,
			capabilities: request.capabilities,
			resizeLease: request.requestResizeLease,
			terminalId: identity.terminalId,
		} as never;
	});
	const host = { requestControl } as unknown as HostConnection;
	const brokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => Promise<void>>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const cleanupCallbacks: Array<((error: unknown) => void) | undefined> = [];
	let nextAttachment = 1;
	const createMessageChannel = vi.fn<
		DesktopTerminalProjectionServiceDependencies['createMessageChannel']
	>(() => ({
		port1: {} as MessagePortMain,
		port2: {} as MessagePortMain,
	}));
	const createBroker = vi.fn<DesktopTerminalProjectionServiceDependencies['createBroker']>(
		(_host, _port, bootstrap, _effectTarget, lifecycle) => {
			const broker = {
				dispose: vi.fn<() => Promise<void>>().mockImplementation(async () => {
					await requestControl(
						MessageKind.detachTerminal,
						{ attachmentId: bootstrap.attachmentId },
						AttachmentAcknowledgedSchema,
					);
				}),
				metrics: EMPTY_METRICS,
				start: vi.fn<() => void>(),
			};
			cleanupCallbacks.push(lifecycle?.onCleanupRequested);
			brokers.push(broker);
			return broker;
		},
	);
	const service = new DesktopTerminalProjectionService({
		createBroker,
		createMessageChannel,
		getProjectionTarget: () => target,
		host,
		randomAttachmentId: () => (nextAttachment++).toString(16).padStart(32, '0'),
	});
	return {
		brokers,
		cleanupCallbacks,
		createBroker,
		createMessageChannel,
		requestControl,
		service,
		setTarget: (nextTarget: typeof target) => {
			target = nextTarget;
		},
	};
}

describe('DesktopTerminalProjectionService', () => {
	it('opens independent scoped attachments and retires only the named presentation', async () => {
		const harness = createHarness();
		const firstPresentation = terminalPresentationId('presentation-1');
		const secondPresentation = terminalPresentationId('presentation-2');

		const first = await harness.service.open(firstPresentation);
		const second = await harness.service.open(secondPresentation);

		expect(harness.brokers).toHaveLength(2);
		expect(harness.brokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.brokers[1]?.start).toHaveBeenCalledOnce();
		await harness.service.retirePresentation(firstPresentation);
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.brokers[1]?.dispose).not.toHaveBeenCalled();
		await second.dispose();
		await first.dispose();
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledOnce();
	});

	it('requests read-only capabilities for an exited presentation', async () => {
		const harness = createHarness(true);

		const lease = await harness.service.open(terminalPresentationId('presentation-exited'));

		expect(harness.requestControl).toHaveBeenCalledWith(
			MessageKind.attachTerminal,
			expect.objectContaining({
				capabilities: { input: false, read: true, resize: false },
				requestResizeLease: false,
			}),
			expect.anything(),
		);
		expect(lease.bootstrap).toMatchObject({ canInput: false, canResize: false });
	});

	it('detaches a Host-applied attachment when the attach response is rejected', async () => {
		const harness = createHarness();
		const defaultImplementation = harness.requestControl.getMockImplementation();
		let hostAttachmentApplied = false;
		let detachFailuresRemaining = 1;
		harness.requestControl.mockImplementation(async (...arguments_) => {
			if (arguments_[0] === MessageKind.attachTerminal) {
				hostAttachmentApplied = true;
				throw new Error('invalid attach response');
			}
			if (arguments_[0] === MessageKind.detachTerminal && detachFailuresRemaining > 0) {
				if (!hostAttachmentApplied) throw new Error('Host attachment was not applied.');
				detachFailuresRemaining -= 1;
				throw new Error('detach unavailable');
			}
			if (defaultImplementation === undefined) throw new Error('Missing Host response.');
			return defaultImplementation(...arguments_);
		});

		await expect(
			harness.service.open(terminalPresentationId('presentation-attach-rejected')),
		).rejects.toMatchObject({
			failure: { code: 'terminal-projection-cleanup-failed', retryable: true },
		});
		await expect(harness.service.dispose()).resolves.toBeUndefined();
		expect(harness.requestControl).toHaveBeenCalledTimes(3);
	});

	it('retains a mismatched detach ACK as a retryable cleanup owner', async () => {
		const harness = createHarness();
		const attachResponse = createDeferred<void>();
		const defaultImplementation = harness.requestControl.getMockImplementation();
		let mismatchedAck = true;
		harness.requestControl.mockImplementation(async (...arguments_) => {
			if (arguments_[0] === MessageKind.attachTerminal) await attachResponse.promise;
			if (arguments_[0] === MessageKind.detachTerminal && mismatchedAck) {
				mismatchedAck = false;
				return { attachmentId: 'f'.repeat(32) } as never;
			}
			if (defaultImplementation === undefined) throw new Error('Missing Host response.');
			return defaultImplementation(...arguments_);
		});
		const opening = harness.service.open(terminalPresentationId('presentation-ack-mismatch'));
		await vi.waitFor(() => expect(harness.requestControl).toHaveBeenCalledOnce());
		harness.setTarget({
			identity: {
				creationId: CreationIdSchema.parse('22'.repeat(16)),
				hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
				terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			},
			readOnly: true,
			terminalOutcome: {
				event: {
					exit: { code: 1, kind: 'exit-code' },
					finalRevision: CanonicalUint64TextSchema.parse('7'),
					identity: {
						creationId: CreationIdSchema.parse('22'.repeat(16)),
						hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
						terminalId: TerminalIdSchema.parse('44'.repeat(16)),
					},
				},
				kind: 'exited',
			},
		});
		attachResponse.resolve();

		await expect(opening).rejects.toMatchObject({
			failure: { code: 'terminal-projection-cleanup-failed', retryable: true },
		});
		await expect(
			harness.service.retirePresentation(terminalPresentationId('presentation-ack-mismatch')),
		).resolves.toBeUndefined();
		expect(harness.requestControl).toHaveBeenCalledTimes(3);
	});

	it('grants one resize owner and keeps later clients as input-capable observers', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-shared');

		const first = await harness.service.open(presentation);
		const second = await harness.service.open(presentation);

		expect(first.bootstrap).toMatchObject({ canInput: true, canResize: true });
		expect(second.bootstrap).toMatchObject({ canInput: true, canResize: false });
		expect(harness.requestControl).toHaveBeenNthCalledWith(
			1,
			MessageKind.attachTerminal,
			expect.objectContaining({
				capabilities: { input: true, read: true, resize: true },
				requestResizeLease: true,
			}),
			expect.anything(),
		);
		expect(harness.requestControl).toHaveBeenNthCalledWith(
			2,
			MessageKind.attachTerminal,
			expect.objectContaining({
				capabilities: { input: true, read: true, resize: false },
				requestResizeLease: false,
			}),
			expect.anything(),
		);

		await first.dispose();
		const nextOwner = await harness.service.open(presentation);
		expect(second.bootstrap.canResize).toBe(false);
		expect(nextOwner.bootstrap.canResize).toBe(true);
	});

	it('serializes concurrent opens so they cannot both request the resize lease', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-concurrent');
		let releaseFirst: (() => void) | undefined;
		const firstResponse = new Promise<void>((resolve) => {
			releaseFirst = resolve;
		});
		const defaultImplementation = harness.requestControl.getMockImplementation();
		harness.requestControl.mockImplementationOnce(async (...arguments_) => {
			await firstResponse;
			if (defaultImplementation === undefined) throw new Error('Missing Host response.');
			return defaultImplementation(...arguments_);
		});

		const first = harness.service.open(presentation);
		const second = harness.service.open(presentation);
		await vi.waitFor(() => expect(harness.requestControl).toHaveBeenCalledOnce());
		await Promise.resolve();
		expect(harness.requestControl).toHaveBeenCalledOnce();
		releaseFirst?.();

		expect((await first).bootstrap.canResize).toBe(true);
		expect((await second).bootstrap.canResize).toBe(false);
	});

	it('detaches an attach whose target becomes exited before the capabilities response is consumed', async () => {
		const harness = createHarness();
		const attachResponse = createDeferred<void>();
		const defaultImplementation = harness.requestControl.getMockImplementation();
		harness.requestControl.mockImplementation(async (...arguments_) => {
			if (arguments_[0] === MessageKind.attachTerminal) await attachResponse.promise;
			if (defaultImplementation === undefined) throw new Error('Missing Host response.');
			return defaultImplementation(...arguments_);
		});
		const presentation = terminalPresentationId('presentation-exit-race');
		const opening = harness.service.open(presentation);
		await vi.waitFor(() => expect(harness.requestControl).toHaveBeenCalledOnce());

		const terminalExit = {
			exit: { code: 1, kind: 'exit-code' as const },
			finalRevision: CanonicalUint64TextSchema.parse('7'),
			identity: {
				creationId: CreationIdSchema.parse('22'.repeat(16)),
				hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
				terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			},
		};
		harness.setTarget({
			identity: terminalExit.identity,
			readOnly: true,
			terminalOutcome: { event: terminalExit, kind: 'exited' },
		});
		attachResponse.resolve();

		await expect(opening).rejects.toMatchObject({
			failure: { code: 'terminal-projection-unavailable', retryable: true },
		});
		expect(harness.brokers).toHaveLength(0);
		expect(harness.service.getMetrics()).toEqual([]);
		expect(harness.requestControl).toHaveBeenNthCalledWith(
			2,
			MessageKind.detachTerminal,
			{ attachmentId: '1'.padStart(32, '0') },
			expect.anything(),
		);
	});

	it('denies a new open after retirement starts while an earlier attach is in flight', async () => {
		const harness = createHarness();
		const attachResponse = createDeferred<void>();
		const defaultImplementation = harness.requestControl.getMockImplementation();
		harness.requestControl.mockImplementation(async (...arguments_) => {
			if (arguments_[0] === MessageKind.attachTerminal) await attachResponse.promise;
			if (defaultImplementation === undefined) throw new Error('Missing Host response.');
			return defaultImplementation(...arguments_);
		});
		const presentation = terminalPresentationId('presentation-retiring');
		const first = harness.service.open(presentation);
		await vi.waitFor(() => expect(harness.requestControl).toHaveBeenCalledOnce());

		const retirement = harness.service.retirePresentation(presentation);
		await expect(harness.service.open(presentation)).rejects.toMatchObject({
			failure: { code: 'terminal-projection-retired', retryable: false },
		});
		attachResponse.resolve();

		await expect(first).rejects.toMatchObject({
			failure: { code: 'terminal-projection-retired', retryable: false },
		});
		await expect(retirement).resolves.toBeUndefined();
		expect(harness.brokers).toHaveLength(0);
		expect(harness.requestControl).toHaveBeenNthCalledWith(
			2,
			MessageKind.detachTerminal,
			{ attachmentId: '1'.padStart(32, '0') },
			expect.anything(),
		);
	});

	it('keeps failed stale-attach cleanup observable and retries it before retirement completes', async () => {
		const harness = createHarness();
		const attachResponse = createDeferred<void>();
		let detachFailuresRemaining = 2;
		const defaultImplementation = harness.requestControl.getMockImplementation();
		harness.requestControl.mockImplementation(async (...arguments_) => {
			if (arguments_[0] === MessageKind.attachTerminal) await attachResponse.promise;
			if (arguments_[0] === MessageKind.detachTerminal && detachFailuresRemaining > 0) {
				detachFailuresRemaining -= 1;
				throw new Error('detach unavailable');
			}
			if (defaultImplementation === undefined) throw new Error('Missing Host response.');
			return defaultImplementation(...arguments_);
		});
		const presentation = terminalPresentationId('presentation-cleanup-retry');
		const opening = harness.service.open(presentation);
		await vi.waitFor(() => expect(harness.requestControl).toHaveBeenCalledOnce());
		const retirement = harness.service.retirePresentation(presentation);
		attachResponse.resolve();

		await expect(opening).rejects.toMatchObject({
			failure: { code: 'terminal-projection-cleanup-failed', retryable: true },
		});
		await expect(retirement).rejects.toMatchObject({
			failure: { code: 'terminal-projection-cleanup-failed', retryable: true },
		});
		expect(harness.brokers).toHaveLength(0);

		await expect(harness.service.retirePresentation(presentation)).resolves.toBeUndefined();
		expect(harness.requestControl).toHaveBeenCalledTimes(4);
	});

	it('retains a lease after detach failure and retries cleanup on the next retirement', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-lease-cleanup');
		await harness.service.open(presentation);
		harness.brokers[0]?.dispose.mockRejectedValueOnce(new Error('detach unavailable'));

		await expect(harness.service.retirePresentation(presentation)).rejects.toThrow(
			'detach unavailable',
		);
		expect(harness.service.getMetrics()).toHaveLength(1);

		await expect(harness.service.retirePresentation(presentation)).resolves.toBeUndefined();
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledTimes(2);
		expect(harness.service.getMetrics()).toEqual([]);
	});

	it('keeps an automatically requested broker cleanup owned until retirement retries it', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-automatic-cleanup');
		await harness.service.open(presentation);
		harness.brokers[0]?.dispose.mockRejectedValueOnce(new Error('detach unavailable'));

		harness.cleanupCallbacks[0]?.(new Error('renderer port closed'));
		await vi.waitFor(() => expect(harness.brokers[0]?.dispose).toHaveBeenCalledOnce());
		expect(harness.service.getMetrics()).toHaveLength(1);

		await expect(harness.service.retirePresentation(presentation)).resolves.toBeUndefined();
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledTimes(2);
		expect(harness.service.getMetrics()).toEqual([]);
	});

	it('detaches an attachment when broker construction fails', async () => {
		const harness = createHarness();
		harness.createBroker.mockImplementationOnce(() => {
			throw new Error('broker factory failed');
		});

		await expect(
			harness.service.open(terminalPresentationId('presentation-factory-failure')),
		).rejects.toThrow('broker factory failed');
		expect(harness.requestControl).toHaveBeenNthCalledWith(
			2,
			MessageKind.detachTerminal,
			expect.objectContaining({ attachmentId: expect.any(String) }),
			AttachmentAcknowledgedSchema,
		);
		expect(harness.service.getMetrics()).toEqual([]);
	});

	it('detaches an attachment when message-channel creation fails', async () => {
		const harness = createHarness();
		harness.createMessageChannel.mockImplementationOnce(() => {
			throw new Error('message channel failed');
		});

		await expect(
			harness.service.open(terminalPresentationId('presentation-channel-failure')),
		).rejects.toThrow('message channel failed');
		expect(harness.createBroker).not.toHaveBeenCalled();
		expect(harness.requestControl).toHaveBeenNthCalledWith(
			2,
			MessageKind.detachTerminal,
			expect.objectContaining({ attachmentId: expect.any(String) }),
			AttachmentAcknowledgedSchema,
		);
	});

	it('retains a factory-failure detach for service disposal retry', async () => {
		const harness = createHarness();
		harness.createBroker.mockImplementationOnce(() => {
			throw new Error('broker factory failed');
		});
		let detachFailuresRemaining = 1;
		const defaultImplementation = harness.requestControl.getMockImplementation();
		harness.requestControl.mockImplementation(async (...arguments_) => {
			if (arguments_[0] === MessageKind.detachTerminal && detachFailuresRemaining > 0) {
				detachFailuresRemaining -= 1;
				throw new Error('detach unavailable');
			}
			if (defaultImplementation === undefined) throw new Error('Missing Host response.');
			return defaultImplementation(...arguments_);
		});

		const opening = harness.service.open(terminalPresentationId('presentation-factory-retry'));
		await expect(opening).rejects.toMatchObject({
			failure: { code: 'terminal-projection-cleanup-failed', retryable: true },
		});
		await expect(harness.service.dispose()).resolves.toBeUndefined();
		expect(harness.requestControl).toHaveBeenCalledTimes(3);
	});

	it('detaches an attachment when broker start fails', async () => {
		const harness = createHarness();
		harness.createBroker.mockImplementationOnce((_host, _port, bootstrap) => {
			const broker = {
				dispose: vi.fn<() => Promise<void>>().mockImplementation(async () => {
					await harness.requestControl(
						MessageKind.detachTerminal,
						{ attachmentId: bootstrap.attachmentId },
						AttachmentAcknowledgedSchema,
					);
				}),
				metrics: EMPTY_METRICS,
				start: vi.fn<() => void>(() => {
					throw new Error('broker start failed');
				}),
			};
			harness.brokers.push(broker);
			return broker;
		});

		await expect(
			harness.service.open(terminalPresentationId('presentation-start-failure')),
		).rejects.toThrow('broker start failed');
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.requestControl).toHaveBeenNthCalledWith(
			2,
			MessageKind.detachTerminal,
			expect.objectContaining({ attachmentId: expect.any(String) }),
			AttachmentAcknowledgedSchema,
		);
		expect(harness.service.getMetrics()).toEqual([]);
	});

	it('rejects a presentation that has no reconciled Host identity', async () => {
		const harness = createHarness();
		const service = new DesktopTerminalProjectionService({
			createBroker: vi.fn<DesktopTerminalProjectionServiceDependencies['createBroker']>(),
			createMessageChannel:
				vi.fn<DesktopTerminalProjectionServiceDependencies['createMessageChannel']>(),
			getProjectionTarget: () => null,
			host: { requestControl: harness.requestControl } as unknown as HostConnection,
			randomAttachmentId: () => '11'.repeat(16),
		});

		await expect(
			service.open(terminalPresentationId('presentation-missing')),
		).rejects.toMatchObject({
			failure: { code: 'terminal-presentation-not-ready', retryable: true },
		});
		expect(harness.requestControl).not.toHaveBeenCalled();
	});
});

function createDeferred<Value>(): { promise: Promise<Value>; resolve: (value: Value) => void } {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}
