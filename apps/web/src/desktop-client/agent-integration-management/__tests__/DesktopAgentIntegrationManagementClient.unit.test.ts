import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopAgentIntegrationManagementMainEventSchema,
	DesktopAgentIntegrationManagementRendererCommandSchema,
	type DesktopAgentIntegrationManagementSnapshot,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import { DesktopAgentIntegrationManagementClient } from '../DesktopAgentIntegrationManagementClient';

class TestMessagePort extends EventTarget {
	closed = false;
	peer: TestMessagePort | undefined;
	starts = 0;

	close(): void {
		this.closed = true;
	}

	postMessage(message: unknown): void {
		if (this.closed) throw new Error('MessagePort is closed.');
		const peer = this.peer;
		queueMicrotask(() => {
			if (peer?.closed === false) {
				peer.dispatchEvent(new MessageEvent('message', { data: message }));
			}
		});
	}

	start(): void {
		this.starts += 1;
	}
}

function createTestChannel(): { readonly port1: MessagePort; readonly port2: TestMessagePort } {
	const port1 = new TestMessagePort();
	const port2 = new TestMessagePort();
	port1.peer = port2;
	port2.peer = port1;
	return { port1: port1 as unknown as MessagePort, port2 };
}

describe('DesktopAgentIntegrationManagementClient', () => {
	it('initializes from inventory and projects one explicit operation at a time', async () => {
		const channel = createTestChannel();
		const commands: unknown[] = [];
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopAgentIntegrationManagementRendererCommandSchema.parse(
				(event as MessageEvent).data,
			);
			commands.push(command);
			const state =
				command.type === 'desktop-agent-integration-enable-requested' ? 'enabled' : 'not-installed';
			channel.port2.postMessage(
				DesktopAgentIntegrationManagementMainEventSchema.parse({
					requestId: command.requestId,
					result: {
						kind: 'completed',
						snapshot: snapshot(state, commands.length),
					},
					type: 'desktop-agent-integration-responded',
				}),
			);
		});
		const client = new DesktopAgentIntegrationManagementClient(channel.port1, () => 'test');
		const listener = vi.fn<() => void>();
		client.subscribe(listener);

		await expect(client.initialize()).resolves.toEqual({ kind: 'ready' });
		expect(client.getSnapshot()).toMatchObject({
			failureCode: null,
			pending: false,
			revision: 1,
		});
		const operation = client.enable('codex');
		expect(client.getSnapshot()).toMatchObject({
			pending: true,
			pendingIntegrationId: 'codex',
		});
		await expect(operation).resolves.toMatchObject({ kind: 'completed' });
		expect(
			client.getSnapshot().integrations.find((item) => item.integrationId === 'codex')?.state,
		).toBe('enabled');
		expect(commands).toMatchObject([
			{ type: 'desktop-agent-integration-inventory-requested' },
			{ integrationId: 'codex', type: 'desktop-agent-integration-enable-requested' },
		]);
		expect(listener).toHaveBeenCalledTimes(3);

		client.dispose();
		expect(listener).toHaveBeenCalledTimes(4);
		expect(channel.port1).toMatchObject({ closed: true });
	});

	it('fails closed when Main sends an invalid or unmatched response', async () => {
		const channel = createTestChannel();
		let responseSequence = 0;
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopAgentIntegrationManagementRendererCommandSchema.parse(
				(event as MessageEvent).data,
			);
			responseSequence += 1;
			channel.port2.postMessage(
				responseSequence === 1
					? DesktopAgentIntegrationManagementMainEventSchema.parse({
							requestId: command.requestId,
							result: { kind: 'completed', snapshot: snapshot('not-installed', 1) },
							type: 'desktop-agent-integration-responded',
						})
					: { requestId: command.requestId, type: 'unexpected-response' },
			);
		});
		const client = new DesktopAgentIntegrationManagementClient(channel.port1, () => 'test');
		await expect(client.initialize()).resolves.toEqual({ kind: 'ready' });

		await expect(client.disable('codex')).resolves.toEqual({ kind: 'unavailable' });
		expect(client.getSnapshot()).toMatchObject({
			failureCode: 'control-unavailable',
			failureIntegrationId: 'codex',
			pending: false,
		});
		expect(channel.port1).toMatchObject({ closed: true });
	});

	it('accepts idempotent launch observations before initialization and during an operation', async () => {
		const channel = createTestChannel();
		let enableRequestId: string | undefined;
		let inventoryRequestId: string | undefined;
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopAgentIntegrationManagementRendererCommandSchema.parse(
				(event as MessageEvent).data,
			);
			if (command.type === 'desktop-agent-integration-inventory-requested') {
				inventoryRequestId = command.requestId;
				return;
			}
			enableRequestId = command.requestId;
		});
		const client = new DesktopAgentIntegrationManagementClient(channel.port1, () => 'test');
		const listener = vi.fn<() => void>();
		client.subscribe(listener);
		const initialization = client.initialize();
		await vi.waitFor(() => expect(inventoryRequestId).toBeDefined());

		channel.port2.postMessage({
			integration: integration('grok'),
			integrationId: 'grok',
			revision: 2,
			type: 'desktop-agent-integration-launch-observed',
		});
		if (inventoryRequestId === undefined) throw new Error('Expected an inventory request.');
		channel.port2.postMessage(
			DesktopAgentIntegrationManagementMainEventSchema.parse({
				requestId: inventoryRequestId,
				result: { kind: 'completed', snapshot: snapshot('not-installed', 1) },
				type: 'desktop-agent-integration-responded',
			}),
		);
		await expect(initialization).resolves.toEqual({ kind: 'ready' });
		await vi.waitFor(() =>
			expect(client.getSnapshot()).toMatchObject({
				launchedIntegrationIds: ['grok'],
				revision: 2,
			}),
		);
		channel.port2.postMessage({
			integration: integration('grok'),
			integrationId: 'grok',
			revision: 2,
			type: 'desktop-agent-integration-launch-observed',
		});
		const enabling = client.enable('codex');
		channel.port2.postMessage({
			integration: integration('claude-code'),
			integrationId: 'claude-code',
			revision: 3,
			type: 'desktop-agent-integration-launch-observed',
		});
		await vi.waitFor(() =>
			expect(client.getSnapshot()).toMatchObject({
				launchedIntegrationIds: ['grok', 'claude-code'],
				pending: true,
			}),
		);
		if (enableRequestId === undefined) throw new Error('Expected an enable request.');
		channel.port2.postMessage(
			DesktopAgentIntegrationManagementMainEventSchema.parse({
				requestId: enableRequestId,
				result: { kind: 'completed', snapshot: snapshot('enabled', 2) },
				type: 'desktop-agent-integration-responded',
			}),
		);
		await expect(enabling).resolves.toMatchObject({ kind: 'completed' });
		expect(client.getSnapshot()).toMatchObject({
			launchedIntegrationIds: ['grok', 'claude-code'],
			revision: 3,
		});
		expect(listener).toHaveBeenCalledTimes(4);

		client.dispose();
	});
});

function snapshot(
	codexState: 'enabled' | 'not-installed',
	revision: number,
): DesktopAgentIntegrationManagementSnapshot {
	return {
		integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
			availability: integrationId === 'codex' ? 'available' : 'not-found',
			integrationId,
			issue: null,
			managementMode: 'managed-local',
			state: integrationId === 'codex' ? codexState : 'unknown',
		})),
		launchedIntegrationIds: [],
		revision,
	};
}

function integration(integrationId: (typeof DESKTOP_AGENT_INTEGRATION_IDS)[number]) {
	const item = snapshot('not-installed', 0).integrations.find(
		(candidate) => candidate.integrationId === integrationId,
	);
	if (item === undefined) throw new Error('Missing test integration.');
	return item;
}
