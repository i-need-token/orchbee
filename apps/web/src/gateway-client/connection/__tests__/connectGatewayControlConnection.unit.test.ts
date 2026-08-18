import {
	encodeGatewayControlServerMessage,
	GatewayBrowserConnectionSchema,
	GatewayControlServerMessageSchema,
} from '@orchbee/gateway-protocol';
import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
} from '@orchbee/workbench-client-protocol';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { describe, expect, it } from 'vitest';

import { connectGatewayControlConnection } from '../connectGatewayControlConnection';

const document = createWorkbenchSharedDocument({
	projectWorkspace: {
		nextProjectSequence: 1,
		nextTerminalContextSequence: 2,
		nextWorkspaceSequence: 1,
		projects: [],
		standaloneTerminalContextId: 'terminal-context-1',
	},
	revision: workbenchSharedDocumentRevision('0'),
	terminalContexts: [
		{
			nextTabSequence: 1,
			panelWorkspace: {
				nextPanelSequence: 1,
				nextPanelSlotSequence: 1,
				nextSplitSequence: 1,
				nextTerminalSequence: 1,
				pendingRemovals: [],
				tabs: [],
			},
			tabCollection: { groups: [], tabs: [] },
			terminalContextId: 'terminal-context-1',
		},
	],
});
const bootstrap = WorkbenchClientSessionBootstrapSchema.parse({
	clientSessionId: 'client-session-1',
	defaultCwd: '/workspace',
	documentState: { document, kind: 'available' },
	initialPresentationId: 'presentation-initial',
	initializationDocument: document,
	sequence: '0',
	terminalHostName: 'orchbee-host',
	terminalEvents: [],
	type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
});

describe('connectGatewayControlConnection', () => {
	it('authenticates before exposing bootstrap and routes channel-scoped events', async () => {
		const socket = new FakeWebSocket();
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.1',
		});
		const connecting = connectGatewayControlConnection(connection, '192.168.1.14', (url) => {
			expect(url).toBe('ws://192.168.1.14:42000/v1/control');
			return socket.asWebSocket();
		});
		socket.open();
		expect(socket.sent[0]).toContain('gateway-control-authenticate');
		socket.receive(
			encodeGatewayControlServerMessage(
				GatewayControlServerMessageSchema.parse({
					bootstrap,
					type: 'gateway-control-ready',
				}),
			),
		);
		const control = await connecting;
		const publications: unknown[] = [];
		control.subscribeClientSession(
			(event) => publications.push(event),
			() => undefined,
		);
		const publication = {
			document,
			sequence: '1',
			type: 'shared-document-published',
		} as const;
		socket.receive(
			encodeGatewayControlServerMessage(
				GatewayControlServerMessageSchema.parse({
					event: publication,
					type: 'workbench-client-session-event',
				}),
			),
		);

		expect(control.bootstrap).toEqual(bootstrap);
		expect(publications).toEqual([expect.objectContaining(publication)]);
		control.close();
	});

	it('rejects bootstrap when the authentication send fails', async () => {
		const socket = new FakeWebSocket();
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.1',
		});
		const connecting = connectGatewayControlConnection(connection, '192.168.1.14', () =>
			socket.asWebSocket(),
		);
		socket.failNextSend();

		socket.open();

		await expect(connecting).rejects.toThrow('Gateway control connection is unavailable.');
		expect(socket.readyState).toBe(3);
	});
});

class FakeWebSocket extends EventTarget {
	binaryType: BinaryType = 'blob';
	bufferedAmount = 0;
	readyState = 0;
	readonly sent: Array<string | ArrayBufferLike | Blob | ArrayBufferView> = [];
	#sendFailure: Error | undefined;

	asWebSocket(): WebSocket {
		return this as unknown as WebSocket;
	}

	close(): void {
		this.readyState = 3;
	}

	failNextSend(): void {
		this.#sendFailure = new Error('injected WebSocket send failure');
	}

	open(): void {
		this.readyState = 1;
		this.dispatchEvent(new Event('open'));
	}

	receive(data: string | ArrayBuffer): void {
		this.dispatchEvent(new MessageEvent('message', { data }));
	}

	send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void {
		const failure = this.#sendFailure;
		this.#sendFailure = undefined;
		if (failure !== undefined) throw failure;
		this.sent.push(data);
	}
}
