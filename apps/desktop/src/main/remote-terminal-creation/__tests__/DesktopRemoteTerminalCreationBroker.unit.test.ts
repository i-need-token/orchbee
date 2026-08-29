import { DesktopRemoteTerminalCreationMainEventSchema } from '@orchbee/desktop-client-protocol';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench/document';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopRemoteTerminalCreationBroker,
	type DesktopRemoteTerminalCreationBrokerRuntime,
} from '../DesktopRemoteTerminalCreationBroker';

describe('DesktopRemoteTerminalCreationBroker', () => {
	it('publishes one bounded canonical settlement for the exact target', async () => {
		const port = new FakeMessagePort();
		const runtime = createRuntime(async (_projectId, _workspaceId, signal) => {
			expect(signal.aborted).toBe(false);
			return createdResult();
		});
		const broker = new DesktopRemoteTerminalCreationBroker(port.asMessagePortMain(), runtime);
		broker.start();

		port.emit(request('remote-terminal:1'));
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));

		expect(runtime.create).toHaveBeenCalledWith(
			'project-1',
			'workspace-1',
			expect.any(AbortSignal),
		);
		expect(DesktopRemoteTerminalCreationMainEventSchema.parse(port.messages[0])).toEqual({
			requestId: 'remote-terminal:1',
			result: createdResult(),
			type: 'desktop-remote-terminal-creation-settled',
		});
	});

	it('rejects concurrent identity, deduplicates an exact replay, and confirms cancellation', async () => {
		const port = new FakeMessagePort();
		let finish: (() => void) | undefined;
		const runtime = createRuntime(
			(_projectId, _workspaceId, signal) =>
				new Promise((resolve) => {
					finish = () => resolve(signal.aborted ? { kind: 'cancelled' } : createdResult());
				}),
		);
		const broker = new DesktopRemoteTerminalCreationBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit(request('remote-terminal:1'));
		port.emit(request('remote-terminal:1'));
		port.emit(request('remote-terminal:2'));

		expect(runtime.create).toHaveBeenCalledOnce();
		expect(DesktopRemoteTerminalCreationMainEventSchema.parse(port.messages[0])).toMatchObject({
			requestId: 'remote-terminal:2',
			result: { kind: 'failed', reason: 'busy' },
		});

		port.emit({
			requestId: 'remote-terminal:1',
			type: 'desktop-remote-terminal-creation-cancel-requested',
		});
		finish?.();
		await vi.waitFor(() => expect(port.messages).toHaveLength(2));
		expect(DesktopRemoteTerminalCreationMainEventSchema.parse(port.messages[1])).toMatchObject({
			requestId: 'remote-terminal:1',
			result: { kind: 'cancelled' },
		});
	});

	it('turns port retirement into an abort without publishing a false cancellation', async () => {
		const port = new FakeMessagePort();
		let signal: AbortSignal | undefined;
		let finish: (() => void) | undefined;
		const runtime = createRuntime(
			(_projectId, _workspaceId, operationSignal) =>
				new Promise((resolve) => {
					signal = operationSignal;
					finish = () => resolve({ kind: 'cancelled' });
				}),
		);
		const broker = new DesktopRemoteTerminalCreationBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit(request('remote-terminal:1'));

		port.emitClose();
		expect(signal?.aborted).toBe(true);
		finish?.();
		await Promise.resolve();
		expect(port.messages).toEqual([]);
		expect(port.close).not.toHaveBeenCalled();
	});

	it('retires on malformed, transferred, or colliding authority', () => {
		for (const events of [
			[{ data: { type: 'generic-terminal-create' }, ports: [] }],
			[{ data: request('remote-terminal:1'), ports: [{}] }],
			[
				{ data: request('remote-terminal:1'), ports: [] },
				{
					data: request('remote-terminal:1', 'project-2', 'workspace-2'),
					ports: [],
				},
			],
		]) {
			const port = new FakeMessagePort();
			const broker = new DesktopRemoteTerminalCreationBroker(
				port.asMessagePortMain(),
				createRuntime(() => new Promise(() => undefined)),
			);
			broker.start();
			for (const event of events) port.emitEvent(event);
			expect(port.close).toHaveBeenCalledOnce();
		}
	});
});

function request(requestId: string, projectId = 'project-1', workspaceId = 'workspace-1'): unknown {
	return {
		projectId,
		requestId,
		type: 'desktop-remote-terminal-creation-requested',
		workspaceId,
	};
}

function createdResult() {
	return {
		kind: 'created' as const,
		panelId: 'panel-1',
		presentationId: 'presentation-1',
		projectId: 'project-1',
		revision: workbenchSharedDocumentRevision('2'),
		tabId: 'tab-1',
		terminalContextId: 'terminal-context-1',
		workspaceId: 'workspace-1',
	};
}

function createRuntime(
	create: DesktopRemoteTerminalCreationBrokerRuntime['create'],
): DesktopRemoteTerminalCreationBrokerRuntime {
	return { create: vi.fn<DesktopRemoteTerminalCreationBrokerRuntime['create']>(create) };
}

class FakeMessagePort {
	readonly #closeListeners = new Set<() => void>();
	readonly close = vi.fn<() => void>();
	readonly messages: unknown[] = [];
	readonly #messageListeners = new Set<(event: MessageEvent) => void>();

	asMessagePortMain(): MessagePortMain {
		return this as unknown as MessagePortMain;
	}

	emit(data: unknown): void {
		this.emitEvent({ data, ports: [] });
	}

	emitClose(): void {
		for (const listener of this.#closeListeners) listener();
	}

	emitEvent(event: { readonly data: unknown; readonly ports: readonly unknown[] }): void {
		for (const listener of this.#messageListeners) {
			listener(event as unknown as MessageEvent);
		}
	}

	off(event: string, listener: (...arguments_: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.delete(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.delete(listener as unknown as () => void);
		}
	}

	on(event: string, listener: (...arguments_: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.add(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.add(listener as unknown as () => void);
		}
	}

	postMessage(message: unknown): void {
		this.messages.push(message);
	}

	start(): void {}
}
