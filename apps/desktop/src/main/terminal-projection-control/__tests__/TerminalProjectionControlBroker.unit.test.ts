import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { TerminalProjectionControlCommandSchema } from '../../../shared';
import type {
	DesktopTerminalProjectionLease,
	DesktopTerminalProjectionService,
} from '../DesktopTerminalProjectionService';
import { TerminalProjectionControlBroker } from '../TerminalProjectionControlBroker';

type TerminalProjectionControlCommand = ReturnType<
	typeof TerminalProjectionControlCommandSchema.parse
>;
type CloseListener = () => void;
type MessageListener = (event: MessageEvent) => void;
type PortListenerOperation = (
	event: 'close' | 'message',
	listener: CloseListener | MessageListener,
) => void;

function createTestPort() {
	let closeListener: CloseListener | undefined;
	let messageListener: MessageListener | undefined;
	const posted: Array<{ readonly message: unknown; readonly ports: readonly MessagePortMain[] }> =
		[];
	const close = vi.fn<() => void>();
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
		postMessage: vi.fn<(message: unknown, ports?: readonly MessagePortMain[]) => void>(
			(message, ports = []) => posted.push({ message, ports }),
		),
		start: vi.fn<() => void>(),
	} as unknown as MessagePortMain;
	return {
		close,
		emitClose: () => closeListener?.(),
		emitMessage: (data: unknown) => messageListener?.({ data } as MessageEvent),
		port,
		posted,
	};
}

function openCommand(
	requestId = 'projection-1',
	presentationId = 'presentation-1',
): TerminalProjectionControlCommand {
	return {
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
			creationId: '22'.repeat(16) as DesktopTerminalProjectionLease['bootstrap']['creationId'],
			hostEpoch: '33'.repeat(16) as DesktopTerminalProjectionLease['bootstrap']['hostEpoch'],
			terminalId: '44'.repeat(16) as DesktopTerminalProjectionLease['bootstrap']['terminalId'],
		},
		dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		metrics: {} as DesktopTerminalProjectionLease['metrics'],
		rendererPort: {} as MessagePortMain,
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
		expect(testPort.posted[0]?.ports).toEqual([first.rendererPort]);
		expect(testPort.posted[1]?.ports).toEqual([second.rendererPort]);

		testPort.emitMessage(releaseCommand());
		await vi.waitFor(() => expect(first.dispose).toHaveBeenCalledOnce());
		expect(second.dispose).not.toHaveBeenCalled();
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
		expect(testPort.posted).toEqual([]);
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
