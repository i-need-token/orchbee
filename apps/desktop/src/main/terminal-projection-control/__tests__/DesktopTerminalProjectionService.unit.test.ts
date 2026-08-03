import {
	CreationIdSchema,
	HostEpochSchema,
	MessageKind,
	TerminalIdSchema,
	type AttachmentId,
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
	let nextAttachment = 1;
	const service = new DesktopTerminalProjectionService({
		createBroker: () => {
			const broker = {
				dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
				metrics: EMPTY_METRICS,
				start: vi.fn<() => void>(),
			};
			brokers.push(broker);
			return broker;
		},
		createMessageChannel: () => ({
			port1: {} as MessagePortMain,
			port2: {} as MessagePortMain,
		}),
		getProjectionTarget: () => ({ identity, readOnly, terminalExit: null }),
		host,
		randomAttachmentId: () => (nextAttachment++).toString(16).padStart(32, '0'),
	});
	return { brokers, requestControl, service };
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
			MessageKind.attachmentCapabilities,
			expect.anything(),
		);
		expect(lease.bootstrap).toMatchObject({ canInput: false, canResize: false });
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
			MessageKind.attachmentCapabilities,
			expect.anything(),
		);
		expect(harness.requestControl).toHaveBeenNthCalledWith(
			2,
			MessageKind.attachTerminal,
			expect.objectContaining({
				capabilities: { input: true, read: true, resize: false },
				requestResizeLease: false,
			}),
			MessageKind.attachmentCapabilities,
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
