import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type {
	TerminalDocumentControlCommand,
	TerminalDocumentControlOperationResult,
} from '../../../shared';
import {
	TerminalDocumentControlBroker,
	type TerminalDocumentControlOperations,
} from '../TerminalDocumentControlBroker';

type CloseListener = () => void;
type MessageListener = (event: MessageEvent) => void;
type PortListenerOperation = (
	event: 'close' | 'message',
	listener: CloseListener | MessageListener,
) => void;

function createTestPort() {
	let closeListener: CloseListener | undefined;
	let messageListener: MessageListener | undefined;
	const posted: unknown[] = [];
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
		postMessage: vi.fn<(message: unknown) => void>((message) => {
			posted.push(message);
		}),
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

function provisionCommand(operationId = 'operation-1'): TerminalDocumentControlCommand {
	return {
		launchCwd: '/workspace/orchbee',
		operationId: operationId as TerminalDocumentControlCommand['operationId'],
		presentationId: 'presentation-1' as TerminalDocumentControlCommand['presentationId'],
		type: 'terminal-provision-requested',
	};
}

function createDeferred<Result>() {
	let resolve = (_result: Result): void => undefined;
	const promise = new Promise<Result>((nextResolve) => {
		resolve = nextResolve;
	});
	return { promise, resolve };
}

describe('TerminalDocumentControlBroker', () => {
	it('executes a valid command and posts only its correlated result', async () => {
		const testPort = createTestPort();
		const execute = vi.fn<TerminalDocumentControlOperations['execute']>(async (command) => ({
			operationId: command.operationId,
			presentationId: command.presentationId,
			type: 'terminal-provision-completed',
		}));
		const broker = new TerminalDocumentControlBroker(testPort.port, { execute });

		broker.start();
		testPort.emitMessage(provisionCommand());

		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));
		expect(execute).toHaveBeenCalledOnce();
		expect(testPort.posted).toEqual([
			{
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				type: 'terminal-provision-completed',
			},
		]);
	});

	it('revokes the document port for structurally invalid commands', () => {
		const testPort = createTestPort();
		const execute = vi.fn<TerminalDocumentControlOperations['execute']>();
		const broker = new TerminalDocumentControlBroker(testPort.port, { execute });
		broker.start();

		testPort.emitMessage({ channel: 'arbitrary-ipc', type: 'send' });

		expect(execute).not.toHaveBeenCalled();
		expect(testPort.close).toHaveBeenCalledOnce();
	});

	it('deduplicates an exact operation and revokes conflicting reuse', async () => {
		const testPort = createTestPort();
		const execute = vi.fn<TerminalDocumentControlOperations['execute']>(async (command) => ({
			operationId: command.operationId,
			presentationId: command.presentationId,
			type: 'terminal-provision-completed',
		}));
		const broker = new TerminalDocumentControlBroker(testPort.port, { execute });
		broker.start();
		const command = provisionCommand();

		testPort.emitMessage(command);
		testPort.emitMessage({ ...command });
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));
		expect(execute).toHaveBeenCalledOnce();
		testPort.emitMessage({
			operationId: command.operationId,
			presentationId: command.presentationId,
			type: 'terminal-close-requested',
		});
		expect(testPort.close).toHaveBeenCalledOnce();
	});

	it('maps execution failure to a fixed correlated retryable result', async () => {
		const testPort = createTestPort();
		const execute = vi.fn<TerminalDocumentControlOperations['execute']>(async () => {
			throw new Error('sensitive internal failure');
		});
		const broker = new TerminalDocumentControlBroker(testPort.port, { execute });
		broker.start();

		testPort.emitMessage(provisionCommand());

		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));
		expect(testPort.posted).toEqual([
			{
				failure: { code: 'document-control-unavailable', retryable: true },
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				type: 'terminal-provision-failed',
			},
		]);
		expect(JSON.stringify(testPort.posted)).not.toContain('sensitive');
	});

	it('keeps independently reordered completions correlated and drops them after disposal', async () => {
		const testPort = createTestPort();
		const first = createDeferred<TerminalDocumentControlOperationResult>();
		const second = createDeferred<TerminalDocumentControlOperationResult>();
		const execute = vi.fn<TerminalDocumentControlOperations['execute']>((command) =>
			command.operationId === provisionCommand().operationId ? first.promise : second.promise,
		);
		const broker = new TerminalDocumentControlBroker(testPort.port, { execute });
		broker.start();
		const firstCommand = provisionCommand();
		const secondCommand = provisionCommand('operation-2');
		testPort.emitMessage(firstCommand);
		testPort.emitMessage(secondCommand);

		second.resolve({
			operationId: secondCommand.operationId,
			presentationId: secondCommand.presentationId,
			type: 'terminal-provision-completed',
		});
		await vi.waitFor(() => expect(testPort.posted).toHaveLength(1));
		expect(testPort.posted[0]).toMatchObject({ operationId: 'operation-2' });
		broker.dispose();
		first.resolve({
			operationId: firstCommand.operationId,
			presentationId: firstCommand.presentationId,
			type: 'terminal-provision-completed',
		});
		await Promise.resolve();
		expect(testPort.posted).toHaveLength(1);
	});

	it('revokes the port when an operation adapter returns a foreign result', async () => {
		const testPort = createTestPort();
		const execute = vi.fn<TerminalDocumentControlOperations['execute']>(async () =>
			Promise.resolve({
				operationId: 'foreign-operation',
				presentationId: 'presentation-1',
				type: 'terminal-provision-completed',
			} as unknown as TerminalDocumentControlOperationResult),
		);
		const broker = new TerminalDocumentControlBroker(testPort.port, { execute });
		broker.start();

		testPort.emitMessage(provisionCommand());

		await vi.waitFor(() => expect(testPort.close).toHaveBeenCalledOnce());
		expect(testPort.posted).toEqual([]);
	});

	it('publishes only typed exits while the document remains active', () => {
		const testPort = createTestPort();
		const broker = new TerminalDocumentControlBroker(testPort.port, {
			execute: vi.fn<TerminalDocumentControlOperations['execute']>(),
		});
		broker.start();

		expect(
			broker.publishTerminalExit({
				exitCode: 0,
				presentationId: provisionCommand().presentationId,
				signal: null,
				type: 'terminal-exited',
			}),
		).toBe(true);
		testPort.emitClose();
		expect(
			broker.publishTerminalExit({
				exitCode: 0,
				presentationId: provisionCommand().presentationId,
				signal: null,
				type: 'terminal-exited',
			}),
		).toBe(false);
		expect(testPort.posted).toHaveLength(1);
	});
});
