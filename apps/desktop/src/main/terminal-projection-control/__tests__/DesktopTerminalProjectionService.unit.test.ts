import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	AttachmentAcknowledgedSchema,
	HostEpochSchema,
	MessageKind,
	TerminalIdSchema,
	TerminalThemeStateSchema,
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

const mountedGeometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 } as const;
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

class TestDesktopTerminalProjectionService extends DesktopTerminalProjectionService {
	override open(
		presentationId: Parameters<DesktopTerminalProjectionService['open']>[0],
		geometry: Parameters<DesktopTerminalProjectionService['open']>[1] = mountedGeometry,
		options?: Parameters<DesktopTerminalProjectionService['open']>[2],
	): ReturnType<DesktopTerminalProjectionService['open']> {
		return super.open(presentationId, geometry, options);
	}
}

function createHarness(readOnly = false, startRequired = false) {
	const identity = {
		creationId: CreationIdSchema.parse('22'.repeat(16)),
		hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
		terminalId: TerminalIdSchema.parse('44'.repeat(16)),
	};
	let target: {
		identity: typeof identity;
		readOnly: boolean;
		startRequired: boolean;
		terminalOutcome:
			| { event: TerminalExited; kind: 'exited' }
			| { event: TerminalFailed; kind: 'failed' }
			| null;
		theme: typeof theme;
	} = {
		identity,
		readOnly,
		startRequired,
		terminalOutcome: null,
		theme,
	};
	let requestTransportEnded = false;
	const hostClosedListeners = new Set<(reason: Error) => void>();
	const requestControl = vi.fn<HostConnection['requestControl']>(async (kind, value) => {
		if (requestTransportEnded) throw new Error('Host request transport ended.');
		if (kind === MessageKind.startTerminal) {
			const request = value as { readonly geometry: typeof mountedGeometry };
			return {
				geometry: request.geometry,
				identity,
				transition: 'started',
			} as never;
		}
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
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			resizeLease: request.requestResizeLease,
			terminalId: identity.terminalId,
		} as never;
	});
	const host = {
		onClosed: (listener: (reason: Error) => void) => {
			hostClosedListeners.add(listener);
			return () => hostClosedListeners.delete(listener);
		},
		requestControl,
		get requestTransportEnded() {
			return requestTransportEnded;
		},
	} as unknown as HostConnection;
	const brokers: Array<{
		readonly claimResizeLease: ReturnType<typeof vi.fn<() => Promise<void>>>;
		readonly dispose: ReturnType<typeof vi.fn<() => Promise<void>>>;
		readonly setInputCapability: ReturnType<typeof vi.fn<(input: boolean) => Promise<void>>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const cleanupCallbacks: Array<((error: unknown) => void) | undefined> = [];
	const resizeActivityCallbacks: Array<(() => void) | undefined> = [];
	const resizeLeaseCallbacks: Array<((ownsResize: boolean) => void) | undefined> = [];
	let nextAttachment = 1;
	const createMessageChannel = vi.fn<
		DesktopTerminalProjectionServiceDependencies['createMessageChannel']
	>(() => ({
		port1: {} as MessagePortMain,
		port2: {} as MessagePortMain,
	}));
	const createBroker = vi.fn<DesktopTerminalProjectionServiceDependencies['createBroker']>(
		(_host, _port, bootstrap, _effectTarget, lifecycle) => {
			const brokerIndex = brokers.length;
			const broker = {
				claimResizeLease: vi.fn<() => Promise<void>>().mockImplementation(async () => {
					for (const [index, callback] of resizeLeaseCallbacks.entries()) {
						callback?.(index === brokerIndex);
					}
				}),
				dispose: vi.fn<() => Promise<void>>().mockImplementation(async () => {
					if (requestTransportEnded) return;
					await requestControl(
						MessageKind.detachTerminal,
						{ attachmentId: bootstrap.attachmentId },
						AttachmentAcknowledgedSchema,
					);
				}),
				metrics: EMPTY_METRICS,
				setInputCapability: vi.fn<(input: boolean) => Promise<void>>().mockResolvedValue(undefined),
				start: vi.fn<() => void>(),
			};
			cleanupCallbacks.push(lifecycle?.onCleanupRequested);
			resizeActivityCallbacks.push(lifecycle?.onResizeActivity);
			resizeLeaseCallbacks.push(lifecycle?.onResizeLeaseChanged);
			brokers.push(broker);
			return broker;
		},
	);
	const markTerminalStarted = vi.fn<
		DesktopTerminalProjectionServiceDependencies['markTerminalStarted']
	>((_presentationId, startedIdentity) => {
		if (startedIdentity.terminalId !== target.identity.terminalId) return false;
		target = { ...target, startRequired: false };
		return true;
	});
	const service = new TestDesktopTerminalProjectionService({
		createBroker,
		createMessageChannel,
		getProjectionTarget: () => target,
		host,
		markTerminalStarted,
		observeTerminalTheme: (_presentationId, nextTheme) => {
			target = { ...target, theme: nextTheme };
			return true;
		},
		randomAttachmentId: () => (nextAttachment++).toString(16).padStart(32, '0'),
	});
	return {
		brokers,
		cleanupCallbacks,
		createBroker,
		createMessageChannel,
		endRequestTransport: (reason = new Error('Host request transport ended.')) => {
			requestTransportEnded = true;
			for (const listener of hostClosedListeners) listener(reason);
		},
		markTerminalStarted,
		requestControl,
		resizeActivityCallbacks,
		resizeLeaseCallbacks,
		service,
		setTarget: (nextTarget: typeof target) => {
			target = nextTarget;
		},
	};
}

describe('DesktopTerminalProjectionService', () => {
	it('serializes a live input capability mutation before lease disposal', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-live-input-capability');
		const lease = await harness.service.open(presentation);
		let finishMutation: () => void = () => undefined;
		harness.brokers[0]?.setInputCapability.mockImplementationOnce(
			() =>
				new Promise<void>((resolve) => {
					finishMutation = resolve;
				}),
		);

		const mutation = lease.setInputCapability(false);
		const disposal = lease.dispose();
		await Promise.resolve();
		expect(harness.brokers[0]?.setInputCapability).toHaveBeenCalledWith(false);
		expect(harness.brokers[0]?.dispose).not.toHaveBeenCalled();

		finishMutation();
		await mutation;
		await disposal;
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledOnce();
	});

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

	it('opens one post-retirement capture lease with permanently read-only Host capabilities', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-archive-capture');
		const identity = {
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
		};
		await harness.service.retirePresentation(presentation);

		const lease = await harness.service.openCaptureReadOnly(presentation, identity);
		expect(lease.bootstrap).toMatchObject({ canInput: false, canResize: false });
		expect(harness.requestControl).toHaveBeenCalledWith(
			MessageKind.attachTerminal,
			expect.objectContaining({
				capabilities: { input: false, read: true, resize: false },
				requestResizeLease: false,
			}),
			expect.anything(),
		);
		await expect(lease.setInputCapability(true)).rejects.toMatchObject({
			failure: { code: 'terminal-projection-capability-rejected', retryable: false },
		});
		expect(harness.brokers[0]?.setInputCapability).not.toHaveBeenCalled();
		await lease.dispose();
	});

	it('starts a pending terminal with exact mounted geometry before attaching it once', async () => {
		const harness = createHarness(false, true);
		const presentation = terminalPresentationId('presentation-awaiting-start');

		const first = await harness.service.open(presentation, mountedGeometry);
		const second = await harness.service.open(presentation, mountedGeometry);

		expect(harness.requestControl).toHaveBeenNthCalledWith(
			1,
			MessageKind.startTerminal,
			{ geometry: mountedGeometry, terminalId: '44'.repeat(16) },
			expect.anything(),
		);
		expect(harness.requestControl).toHaveBeenNthCalledWith(
			2,
			MessageKind.attachTerminal,
			expect.objectContaining({ terminalId: '44'.repeat(16) }),
			expect.anything(),
		);
		expect(harness.markTerminalStarted).toHaveBeenCalledOnce();
		expect(
			harness.requestControl.mock.calls.filter(([kind]) => kind === MessageKind.startTerminal),
		).toHaveLength(1);
		await second.dispose();
		await first.dispose();
	});

	it('does not attach when starting a pending terminal fails', async () => {
		const harness = createHarness(false, true);
		harness.requestControl.mockRejectedValueOnce(new Error('start unavailable'));

		await expect(
			harness.service.open(terminalPresentationId('presentation-start-rejected'), mountedGeometry),
		).rejects.toThrow('start unavailable');
		expect(harness.requestControl).toHaveBeenCalledOnce();
		expect(harness.createBroker).not.toHaveBeenCalled();
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
			startRequired: false,
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
			theme,
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

	it('promotes the existing observer after its resize owner detaches', async () => {
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
				capabilities: { input: true, read: true, resize: true },
				requestResizeLease: false,
			}),
			expect.anything(),
		);

		await first.dispose();
		harness.resizeLeaseCallbacks[1]?.(true);
		const nextObserver = await harness.service.open(presentation);
		expect(second.bootstrap.canResize).toBe(false);
		expect(nextObserver.bootstrap.canResize).toBe(false);
		expect(harness.requestControl).toHaveBeenLastCalledWith(
			MessageKind.attachTerminal,
			expect.objectContaining({ requestResizeLease: false }),
			expect.anything(),
		);
	});

	it('coalesces activity while a claim is in flight and converges to the latest active projection', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-active-resize-owner');
		await harness.service.open(presentation);
		await harness.service.open(presentation);
		const secondClaim = createDeferred<void>();
		harness.brokers[1]?.claimResizeLease.mockImplementationOnce(async () => {
			await secondClaim.promise;
			harness.resizeLeaseCallbacks[0]?.(false);
			harness.resizeLeaseCallbacks[1]?.(true);
		});

		harness.resizeActivityCallbacks[1]?.();
		harness.resizeActivityCallbacks[1]?.();
		await vi.waitFor(() => expect(harness.brokers[1]?.claimResizeLease).toHaveBeenCalledOnce());
		harness.resizeActivityCallbacks[0]?.();
		expect(harness.brokers[0]?.claimResizeLease).not.toHaveBeenCalled();

		secondClaim.resolve(undefined);
		await vi.waitFor(() => expect(harness.brokers[0]?.claimResizeLease).toHaveBeenCalledOnce());
		expect(harness.brokers[1]?.claimResizeLease).toHaveBeenCalledOnce();
	});

	it('retries recorded newer activity after Host fallback without spinning on claim failure', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-resize-owner-fallback');
		await harness.service.open(presentation);
		await harness.service.open(presentation);
		await harness.service.open(presentation);
		harness.brokers[1]?.claimResizeLease.mockRejectedValueOnce(new Error('injected claim failure'));

		harness.resizeActivityCallbacks[1]?.();
		await vi.waitFor(() => expect(harness.brokers[1]?.claimResizeLease).toHaveBeenCalledOnce());
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(harness.brokers[1]?.claimResizeLease).toHaveBeenCalledOnce();

		harness.resizeLeaseCallbacks[0]?.(false);
		harness.resizeLeaseCallbacks[2]?.(true);
		await vi.waitFor(() => expect(harness.brokers[1]?.claimResizeLease).toHaveBeenCalledTimes(2));
		expect(harness.brokers[2]?.claimResizeLease).not.toHaveBeenCalled();
	});

	it('orders a release before a racing open so the replacement requests ownership', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-release-open-race');
		const owner = await harness.service.open(presentation);
		const detach = createDeferred<void>();
		const defaultImplementation = harness.requestControl.getMockImplementation();
		harness.requestControl.mockImplementation(async (...arguments_) => {
			if (arguments_[0] === MessageKind.detachTerminal) await detach.promise;
			if (defaultImplementation === undefined) throw new Error('Missing Host response.');
			return defaultImplementation(...arguments_);
		});

		const releasing = owner.dispose();
		await vi.waitFor(() => expect(harness.requestControl).toHaveBeenCalledTimes(2));
		const opening = harness.service.open(presentation);
		await Promise.resolve();
		expect(harness.requestControl).toHaveBeenCalledTimes(2);
		detach.resolve();

		await releasing;
		expect((await opening).bootstrap.canResize).toBe(true);
		expect(harness.requestControl).toHaveBeenLastCalledWith(
			MessageKind.attachTerminal,
			expect.objectContaining({ requestResizeLease: true }),
			expect.anything(),
		);
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
			startRequired: false,
			terminalOutcome: { event: terminalExit, kind: 'exited' },
			theme,
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

	it('continues serialized retirement after one lease detach fails', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-multi-lease-cleanup');
		await harness.service.open(presentation);
		await harness.service.open(presentation);
		harness.brokers[0]?.dispose.mockRejectedValueOnce(new Error('detach unavailable'));

		await expect(harness.service.retirePresentation(presentation)).rejects.toThrow(
			'detach unavailable',
		);
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.brokers[1]?.dispose).toHaveBeenCalledOnce();
		expect(harness.service.getMetrics()).toHaveLength(1);

		await expect(harness.service.retirePresentation(presentation)).resolves.toBeUndefined();
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledTimes(2);
		expect(harness.brokers[1]?.dispose).toHaveBeenCalledOnce();
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

	it('settles a failed active lease when the Host request transport ends', async () => {
		const harness = createHarness();
		const presentation = terminalPresentationId('presentation-host-close-cleanup');
		await harness.service.open(presentation);
		harness.brokers[0]?.dispose.mockRejectedValueOnce(new Error('detach unavailable'));

		harness.cleanupCallbacks[0]?.(new Error('renderer port closed'));
		await vi.waitFor(() => expect(harness.brokers[0]?.dispose).toHaveBeenCalledOnce());
		expect(harness.service.getMetrics()).toHaveLength(1);

		harness.endRequestTransport();
		await vi.waitFor(() => expect(harness.service.getMetrics()).toEqual([]));
		expect(harness.brokers[0]?.dispose).toHaveBeenCalledTimes(2);
		expect(
			harness.requestControl.mock.calls.filter(([kind]) => kind === MessageKind.detachTerminal),
		).toEqual([]);
	});

	it('rejects new projection work locally after the Host request transport ends', async () => {
		const harness = createHarness();
		harness.endRequestTransport();
		const requestCountAtEnd = harness.requestControl.mock.calls.length;

		await expect(
			harness.service.open(terminalPresentationId('presentation-after-host-close')),
		).rejects.toMatchObject({
			failure: { code: 'terminal-projection-unavailable', retryable: true },
		});
		expect(harness.requestControl).toHaveBeenCalledTimes(requestCountAtEnd);
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

	it('settles a retained provisional detach when the Host request transport ends', async () => {
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

		await expect(
			harness.service.open(terminalPresentationId('presentation-factory-host-close')),
		).rejects.toMatchObject({
			failure: { code: 'terminal-projection-cleanup-failed', retryable: true },
		});
		const detachCallsBeforeClose = harness.requestControl.mock.calls.filter(
			([kind]) => kind === MessageKind.detachTerminal,
		).length;

		harness.endRequestTransport();
		await expect(harness.service.dispose()).resolves.toBeUndefined();
		expect(
			harness.requestControl.mock.calls.filter(([kind]) => kind === MessageKind.detachTerminal),
		).toHaveLength(detachCallsBeforeClose);
	});

	it('detaches an attachment when broker start fails', async () => {
		const harness = createHarness();
		harness.createBroker.mockImplementationOnce((_host, _port, bootstrap) => {
			const broker = {
				claimResizeLease: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
				dispose: vi.fn<() => Promise<void>>().mockImplementation(async () => {
					await harness.requestControl(
						MessageKind.detachTerminal,
						{ attachmentId: bootstrap.attachmentId },
						AttachmentAcknowledgedSchema,
					);
				}),
				metrics: EMPTY_METRICS,
				setInputCapability: vi.fn<(input: boolean) => Promise<void>>().mockResolvedValue(undefined),
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
			host: {
				onClosed: () => () => undefined,
				requestControl: harness.requestControl,
				requestTransportEnded: false,
			} as unknown as HostConnection,
			markTerminalStarted: () => false,
			observeTerminalTheme: () => false,
			randomAttachmentId: () => '11'.repeat(16),
		});

		await expect(
			service.open(terminalPresentationId('presentation-missing'), mountedGeometry),
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
