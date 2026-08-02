// @vitest-environment jsdom

import { act } from 'react';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import {
	TerminalDocumentControlBootstrapSchema,
	type TerminalDocumentControlCommand,
} from '../../../shared';
import { startDesktopWorkbenchRenderer } from '../startDesktopWorkbenchRenderer';

const originalResizeObserver = globalThis.ResizeObserver;

class ResizeObserverMock {
	disconnect(): void {
		return;
	}

	observe(): void {
		return;
	}

	unobserve(): void {
		return;
	}
}

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
	Object.defineProperty(globalThis, 'ResizeObserver', {
		configurable: true,
		value: ResizeObserverMock,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
	Object.defineProperty(globalThis, 'ResizeObserver', {
		configurable: true,
		value: originalResizeObserver,
	});
});

class TestMessagePort extends EventTarget {
	peer: TestMessagePort | undefined;
	closed = false;

	close(): void {
		this.closed = true;
	}

	postMessage(message: unknown, transfer: Transferable[] = []): void {
		if (this.closed) throw new Error('MessagePort is closed.');
		const peer = this.peer;
		queueMicrotask(() => {
			if (peer?.closed === false) {
				peer.dispatchEvent(
					new MessageEvent('message', { data: message, ports: transfer as MessagePort[] }),
				);
			}
		});
	}

	start(): void {
		return;
	}
}

function createTestChannel(): { readonly port1: MessagePort; readonly port2: TestMessagePort } {
	const port1 = new TestMessagePort();
	const port2 = new TestMessagePort();
	port1.peer = port2;
	port2.peer = port1;
	return { port1: port1 as unknown as MessagePort, port2 };
}

describe('startDesktopWorkbenchRenderer', () => {
	it('requests projection only for the active visible terminal and switches demand with the Tab', async () => {
		const documentChannel = createTestChannel();
		const projectionChannel = createTestChannel();
		const topologyChannel = createTestChannel();
		const documentCommands: TerminalDocumentControlCommand[] = [];
		const projectionCommands: Array<{ presentationId?: string; type?: string }> = [];
		documentChannel.port2.addEventListener('message', (event) => {
			documentCommands.push((event as MessageEvent).data as TerminalDocumentControlCommand);
		});
		projectionChannel.port2.addEventListener('message', (event) => {
			projectionCommands.push(
				(event as MessageEvent).data as { presentationId?: string; type?: string },
			);
		});
		const root = document.createElement('div');
		document.body.appendChild(root);
		let session: ReturnType<typeof startDesktopWorkbenchRenderer> | undefined;
		await act(async () => {
			session = startDesktopWorkbenchRenderer(
				root,
				document.documentElement,
				['en-US'],
				'apple',
				window.localStorage,
				TerminalDocumentControlBootstrapSchema.parse({
					defaultCwd: '/workspace',
					initialPresentationId: 'presentation-initial',
					presentations: [
						{ launchCwd: '/one', presentationId: 'presentation-1' },
						{ launchCwd: '/two', presentationId: 'presentation-2' },
					],
					type: 'orchbee.desktop.terminal-document-control-port',
				}),
				documentChannel.port1,
				projectionChannel.port1,
				topologyChannel.port1,
			);
		});
		await vi.waitFor(() => expect(documentCommands).toHaveLength(2));

		await act(async () => {
			for (const command of documentCommands) {
				documentChannel.port2.postMessage({
					operationId: command.operationId,
					presentationId: command.presentationId,
					type: 'terminal-provision-completed',
				});
			}
		});
		await vi.waitFor(() => {
			expect(projectionCommands).toEqual([
				expect.objectContaining({
					presentationId: 'presentation-1',
					type: 'terminal-projection-open-requested',
				}),
			]);
		});

		const secondTab = [...root.querySelectorAll<HTMLElement>('[role="tab"]')].find((tab) =>
			tab.getAttribute('aria-label')?.includes('Terminal 2'),
		);
		expect(secondTab).toBeDefined();
		await act(async () => {
			secondTab?.dispatchEvent(
				new MouseEvent('pointerdown', { bubbles: true, button: 0, cancelable: true }),
			);
		});
		await vi.waitFor(() => {
			expect(projectionCommands).toEqual(
				expect.arrayContaining([
					expect.objectContaining({
						presentationId: 'presentation-1',
						type: 'terminal-projection-release-requested',
					}),
					expect.objectContaining({
						presentationId: 'presentation-2',
						type: 'terminal-projection-open-requested',
					}),
				]),
			);
		});

		await act(async () => session?.dispose());
		expect((topologyChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		root.remove();
	});
});
