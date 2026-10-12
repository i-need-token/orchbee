import type { MessageEvent, MessagePortMain } from 'electron';
import { TerminalThemeStateSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { TerminalProjectionControlCommandSchema } from '@orchbee/workbench-client-protocol';
import type {
	DesktopTerminalProjectionLease,
	DesktopTerminalProjectionService,
} from '../DesktopTerminalProjectionService';
import { TerminalProjectionControlBroker } from '../TerminalProjectionControlBroker';

type TerminalProjectionControlCommand = ReturnType<
	typeof TerminalProjectionControlCommandSchema.parse
>;
type TerminalProjectionOpenCommand = Extract<
	TerminalProjectionControlCommand,
	{ readonly type: 'terminal-projection-open-requested' }
>;
type CloseListener = () => void;
type MessageListener = (event: MessageEvent) => void;
type PortListenerOperation = (
	event: 'close' | 'message',
	listener: CloseListener | MessageListener,
) => void;

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

function createTestPort() {
	let closeListener: CloseListener | undefined;
	let messageListener: MessageListener | undefined;
	const posted: Array<{ readonly message: unknown; readonly ports: readonly MessagePortMain[] }> =
		[];
	const close = vi.fn<() => void>();
	const postMessage = vi.fn<(message: unknown, ports?: readonly MessagePortMain[]) => void>(
		(message, ports = []) => posted.push({ message, ports }),
	);
	const port = {
		close,
		off: vi.fn<PortListenerOperation>((event, listener) => {
			if (event === 'close' && closeListener === listener) closeListener = undefined;
			if (event === 'message' && messageListener === listener) messageListener = undefined;
		}),
		on: vi.fn<PortListenerOperation>((event, listener) => {
			if (event === 'close') closeListener = listener as CloseListener;
			if (event === 'message') messageListener = listener as MessageListener;
		}),
		postMessage,
		start: vi.fn<() => void>(),
	} as unknown as MessagePortMain;
	return {
		close,
		emitClose: () => closeListener?.(),
		emitMessage: (data: unknown) => messageListener?.({ data } as MessageEvent),
		port,
		postMessage,
		posted,
	};
}

function openCommand(
	requestId = 'projection-1',
	presentationId = 'presentation-1',
	geometry: TerminalProjectionOpenCommand['geometry'] = mountedGeometry,
): TerminalProjectionControlCommand {
	return {
		geometry,
		presentationId: presentationId as TerminalProjectionControlCommand['presentationId'],
		requestId: requestId as TerminalProjectionControlCommand['requestId'],
		type: 'terminal-projection-open-requested',
	};
}

function releaseCommand(
	requestId = 'projection-1',
	presentationId = 'presentation-1',
): TerminalProjectionControlCommand {
	return {
		presentationId: presentationId as TerminalProjectionControlCommand['presentationId'],
		requestId: requestId as TerminalProjectionControlCommand['requestId'],
		type: 'terminal-projection-release-requested',
	};
}

function createLease(): DesktopTerminalProjectionLease {
	return {
		bootstrap: {
			attachmentId: '11'.repeat(16) as DesktopTerminalProjectionLease['bootstrap']['attachmentId'],
			canInput: true,
			canResize: true,
			capabilityRevision: '1' as DesktopTerminalProjectionLease['bootstrap']['capabilityRevision'],
			creationId: '22'.repeat(16) as DesktopTerminalProjectionLease['bootstrap']['creationId'],
			hostEpoch: '33'.repeat(16) as DesktopTerminalProjectionLease['bootstrap']['hostEpoch'],
			terminalId: '44'.repeat(16) as DesktopTerminalProjectionLease['bootstrap']['terminalId'],
			terminalOutcome: null,
			theme,
		},
		dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		metrics: {} as DesktopTerminalProjectionLease['metrics'],
		rendererPort: {} as MessagePortMain,
		setInputCapability: vi.fn<(input: boolean) => Promise<void>>().mockResolvedValue(undefined),
	};
}

function createDeferred<Result>() {
	let resolve = (_result: Result): void => undefined;
	const promise = new Promise<Result>((nextResolve) => (resolve = nextResolve));
	return { promise, resolve };
}

describe('TerminalProjectionControlBroker', () => {
	it('opens and releases unrelated presentation attachments independently', async () => {
		const testPort = createTestPort();
		const first = createLease();
		const second = createLease();
		const open = vi
			.fn<DesktopTerminalProjectionService['open']>()
			.mockResolvedValueOnce(first)
			.mockResolvedValueOnce(second);
		const broker = new TerminalProjectionControlBroker(testPort.port, { open });
		broker.start();

		testPort.emitMessage(openCommand());
		testPort.emitMessage(openCommand('projection-2', 'presentation-2'));
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(2));
		expect(open).toHaveBeenNthCalledWith(1, 'presentation-1', mountedGeometry);
		expect(open).toHaveBeenNthCalledWith(2, 'presentation-2', mountedGeometry);
		expect(testPort.posted[0]?.ports).toEqual([first.rendererPort]);
		expect(testPort.posted[1]?.ports).toEqual([second.rendererPort]);

		testPort.emitMessage(releaseCommand());
		await vi.waitFor(() => expect(first.dispose).toHaveBeenCalledOnce());
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(3));
		expect(second.dispose).not.toHaveBeenCalled();
		expect(testPort.posted[2]?.message).toEqual({
			presentationId: 'presentation-1',
			requestId: 'projection-1',
			type: 'terminal-projection-released',
		});
	});

	it('cancels an in-flight open without publishing a stale renderer port', async () => {
		const testPort = createTestPort();
		const deferred = createDeferred<DesktopTerminalProjectionLease>();
		const broker = new TerminalProjectionControlBroker(testPort.port, {
			open: () => deferred.promise,
		});
		const lease = createLease();
		broker.start();

		testPort.emitMessage(openCommand());
		testPort.emitMessage(releaseCommand());
		deferred.resolve(lease);

		await vi.waitFor(() => expect(lease.dispose).toHaveBeenCalledOnce());
		expect(testPort.posted.map(({ message }) => message)).toEqual([
			{
				presentationId: 'presentation-1',
				requestId: 'projection-1',
				type: 'terminal-projection-released',
			},
		]);
	});

	it('rechecks canonical authority synchronously before publishing a renderer port', async () => {
		const testPort = createTestPort();
		const lease = createLease();
		const isPublicationTargetCurrent = vi.fn<() => boolean>(() => false);
		const broker = new TerminalProjectionControlBroker(testPort.port, {
			isPublicationTargetCurrent,
			open: vi.fn<DesktopTerminalProjectionService['open']>().mockResolvedValue(lease),
		});
		broker.start();

		testPort.emitMessage(openCommand());

		await vi.waitFor(() => expect(lease.dispose).toHaveBeenCalledOnce());
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));
		expect(isPublicationTargetCurrent).toHaveBeenCalledWith('presentation-1', lease);
		expect(testPort.posted[0]).toEqual({
			message: {
				failure: { code: 'terminal-projection-retired', retryable: false },
				presentationId: 'presentation-1',
				requestId: 'projection-1',
				type: 'terminal-projection-failed',
			},
			ports: [],
		});
	});

	it.each(['release', 'retirement'] as const)(
		'retains unpublished cleanup ownership for %s without retiring an unrelated projection',
		async (retry) => {
			const testPort = createTestPort();
			const unpublished = createLease();
			const published = createLease();
			vi.mocked(unpublished.dispose).mockRejectedValueOnce(new Error('detach unavailable'));
			const broker = new TerminalProjectionControlBroker(testPort.port, {
				isPublicationTargetCurrent: (presentationId) => presentationId === 'presentation-2',
				open: vi
					.fn<DesktopTerminalProjectionService['open']>()
					.mockResolvedValueOnce(unpublished)
					.mockResolvedValueOnce(published),
			});
			broker.start();
			testPort.emitMessage(openCommand());
			testPort.emitMessage(openCommand('projection-2', 'presentation-2'));
			await vi.waitFor(() => expect(testPort.posted).toHaveLength(2));
			expect(testPort.posted).toContainEqual({
				message: {
					failure: { code: 'terminal-projection-cleanup-failed', retryable: true },
					presentationId: 'presentation-1',
					requestId: 'projection-1',
					type: 'terminal-projection-failed',
				},
				ports: [],
			});
			const transferredPorts = testPort.posted.flatMap(({ ports }) => ports);
			expect(transferredPorts).toHaveLength(1);
			expect(transferredPorts[0]).toBe(published.rendererPort);
			expect(unpublished.dispose).toHaveBeenCalledOnce();
			expect(published.dispose).not.toHaveBeenCalled();
			const releaseReply = {
				message: {
					presentationId: 'presentation-1',
					requestId: 'projection-1',
					type: 'terminal-projection-released',
				},
				ports: [],
			};
			const expectedReplies = [...testPort.posted, ...(retry === 'release' ? [releaseReply] : [])];

			if (retry === 'release') {
				testPort.emitMessage(releaseCommand());
			} else {
				await broker.dispose();
			}
			await vi.waitFor(() => expect(testPort.posted).toEqual(expectedReplies));
			expect(unpublished.dispose).toHaveBeenCalledTimes(2);
			expect(published.dispose).toHaveBeenCalledTimes(retry === 'release' ? 0 : 1);
			expect(testPort.close).toHaveBeenCalledTimes(retry === 'release' ? 0 : 1);

			await expect(broker.dispose()).resolves.toBeUndefined();
			expect(unpublished.dispose).toHaveBeenCalledTimes(2);
			expect(published.dispose).toHaveBeenCalledOnce();
		},
	);

	it('retains a failed in-flight release cleanup for broker retirement retry', async () => {
		const testPort = createTestPort();
		const deferred = createDeferred<DesktopTerminalProjectionLease>();
		const lease = createLease();
		vi.mocked(lease.dispose)
			.mockRejectedValueOnce(new Error('detach unavailable'))
			.mockResolvedValue(undefined);
		const broker = new TerminalProjectionControlBroker(testPort.port, {
			open: () => deferred.promise,
		});
		broker.start();

		testPort.emitMessage(openCommand());
		testPort.emitMessage(releaseCommand());
		deferred.resolve(lease);

		await vi.waitFor(() => expect(lease.dispose).toHaveBeenCalledOnce());
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));
		expect(testPort.posted[0]?.message).toMatchObject({
			failure: { code: 'terminal-projection-cleanup-failed', retryable: true },
			type: 'terminal-projection-failed',
		});

		testPort.emitMessage(releaseCommand());
		await vi.waitFor(() => expect(lease.dispose).toHaveBeenCalledTimes(2));
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(2));
		await expect(broker.dispose()).resolves.toBeUndefined();
		expect(lease.dispose).toHaveBeenCalledTimes(2);
		expect(testPort.posted[1]?.message).toEqual({
			presentationId: 'presentation-1',
			requestId: 'projection-1',
			type: 'terminal-projection-released',
		});
	});

	it('retains an automatic port-close cleanup failure for later retirement retry', async () => {
		const testPort = createTestPort();
		const lease = createLease();
		vi.mocked(lease.dispose)
			.mockRejectedValueOnce(new Error('detach unavailable'))
			.mockResolvedValue(undefined);
		const broker = new TerminalProjectionControlBroker(testPort.port, {
			open: vi.fn<DesktopTerminalProjectionService['open']>().mockResolvedValue(lease),
		});
		broker.start();
		testPort.emitMessage(openCommand());
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));

		testPort.emitClose();
		await vi.waitFor(() => expect(lease.dispose).toHaveBeenCalledOnce());
		await expect(broker.dispose()).resolves.toBeUndefined();
		expect(lease.dispose).toHaveBeenCalledTimes(2);
	});

	it('retains malformed-control cleanup failure for later retirement retry', async () => {
		const testPort = createTestPort();
		const lease = createLease();
		vi.mocked(lease.dispose)
			.mockRejectedValueOnce(new Error('detach unavailable'))
			.mockResolvedValue(undefined);
		const broker = new TerminalProjectionControlBroker(testPort.port, {
			open: vi.fn<DesktopTerminalProjectionService['open']>().mockResolvedValue(lease),
		});
		broker.start();
		testPort.emitMessage(openCommand());
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));

		testPort.emitMessage({ malformed: true });
		await vi.waitFor(() => expect(lease.dispose).toHaveBeenCalledOnce());
		await expect(broker.dispose()).resolves.toBeUndefined();
		expect(lease.dispose).toHaveBeenCalledTimes(2);
	});

	it('routes an opened-event postMessage failure through cleanup retirement', async () => {
		const testPort = createTestPort();
		const lease = createLease();
		vi.mocked(lease.dispose)
			.mockRejectedValueOnce(new Error('detach unavailable'))
			.mockRejectedValueOnce(new Error('detach still unavailable'))
			.mockResolvedValue(undefined);
		testPort.postMessage.mockImplementationOnce(() => {
			throw new Error('renderer port failed');
		});
		const broker = new TerminalProjectionControlBroker(testPort.port, {
			open: vi.fn<DesktopTerminalProjectionService['open']>().mockResolvedValue(lease),
		});
		broker.start();
		testPort.emitMessage(openCommand());

		await vi.waitFor(() => expect(lease.dispose).toHaveBeenCalledTimes(2));
		await expect(broker.dispose()).resolves.toBeUndefined();
		expect(lease.dispose).toHaveBeenCalledTimes(3);
	});

	it('revokes the document scope for malformed commands and identity reuse', async () => {
		const testPort = createTestPort();
		const lease = createLease();
		const broker = new TerminalProjectionControlBroker(testPort.port, {
			open: vi.fn<DesktopTerminalProjectionService['open']>().mockResolvedValue(lease),
		});
		broker.start();
		testPort.emitMessage(openCommand());
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));

		testPort.emitMessage(releaseCommand('projection-1', 'presentation-foreign'));

		expect(testPort.close).toHaveBeenCalledOnce();
		await vi.waitFor(() => expect(lease.dispose).toHaveBeenCalledOnce());
	});

	it('revokes the document scope when a request identity is reused with new geometry', async () => {
		const testPort = createTestPort();
		const lease = createLease();
		const broker = new TerminalProjectionControlBroker(testPort.port, {
			open: vi.fn<DesktopTerminalProjectionService['open']>().mockResolvedValue(lease),
		});
		broker.start();
		testPort.emitMessage(openCommand());
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));

		testPort.emitMessage(
			openCommand('projection-1', 'presentation-1', { ...mountedGeometry, columns: 97 }),
		);

		await vi.waitFor(() => expect(testPort.close).toHaveBeenCalledOnce());
		await vi.waitFor(() => expect(lease.dispose).toHaveBeenCalledOnce());
	});

	it.each([
		{ failure: new Error('first projection cleanup failure'), kind: 'Error' },
		{ failure: undefined, kind: 'undefined' },
		{ failure: null, kind: 'null' },
	])(
		'preserves the first $kind retirement failure while retaining both leases for retry',
		async ({ failure }) => {
			const testPort = createTestPort();
			const first = createLease();
			const second = createLease();
			vi.mocked(first.dispose).mockRejectedValueOnce(failure);
			vi.mocked(second.dispose).mockRejectedValueOnce(
				new Error('later projection cleanup failure'),
			);
			const broker = new TerminalProjectionControlBroker(testPort.port, {
				open: vi
					.fn<DesktopTerminalProjectionService['open']>()
					.mockResolvedValueOnce(first)
					.mockResolvedValueOnce(second),
			});
			broker.start();
			testPort.emitMessage(openCommand());
			testPort.emitMessage(openCommand('projection-2', 'presentation-2'));
			await vi.waitFor(() => expect(testPort.posted).toHaveLength(2));

			await expect(broker.dispose()).rejects.toBe(failure);
			expect(first.dispose).toHaveBeenCalledOnce();
			expect(second.dispose).toHaveBeenCalledOnce();

			await expect(broker.dispose()).resolves.toBeUndefined();
			expect(first.dispose).toHaveBeenCalledTimes(2);
			expect(second.dispose).toHaveBeenCalledTimes(2);
		},
	);

	it('disposes all attachments when its renderer document closes', async () => {
		const testPort = createTestPort();
		const lease = createLease();
		const broker = new TerminalProjectionControlBroker(testPort.port, {
			open: vi.fn<DesktopTerminalProjectionService['open']>().mockResolvedValue(lease),
		});
		broker.start();
		testPort.emitMessage(openCommand());
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));

		testPort.emitClose();

		await vi.waitFor(() => expect(lease.dispose).toHaveBeenCalledOnce());
		expect(testPort.close).not.toHaveBeenCalled();
	});
});
