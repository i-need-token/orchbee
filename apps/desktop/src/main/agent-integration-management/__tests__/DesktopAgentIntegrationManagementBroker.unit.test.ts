import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopAgentIntegrationManagementMainEventSchema,
	type DesktopAgentIntegrationManagementSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopAgentIntegrationManagementBroker,
	type DesktopAgentIntegrationManagementBrokerController,
} from '../DesktopAgentIntegrationManagementBroker';

const snapshot: DesktopAgentIntegrationManagementSnapshot = {
	integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
		availability: integrationId === 'codex' ? 'available' : 'not-found',
		integrationId,
		issue: null,
		managementMode: 'managed-local',
		state: integrationId === 'codex' ? 'not-installed' : 'unknown',
	})),
	launchedIntegrationIds: [],
	revision: 1,
};

describe('DesktopAgentIntegrationManagementBroker', () => {
	it('serves inventory and routes an explicit bounded mutation', async () => {
		const port = new FakeMessagePort();
		const controller = createController();
		const broker = new DesktopAgentIntegrationManagementBroker(
			port.asMessagePortMain(),
			controller,
		);
		broker.start();

		port.emit({
			requestId: 'agent-integration:request:1',
			type: 'desktop-agent-integration-inventory-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));
		port.emit({
			integrationId: 'codex',
			requestId: 'agent-integration:request:2',
			type: 'desktop-agent-integration-enable-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(2));

		expect(DesktopAgentIntegrationManagementMainEventSchema.parse(port.messages[0])).toEqual({
			requestId: 'agent-integration:request:1',
			result: { kind: 'completed', snapshot },
			type: 'desktop-agent-integration-responded',
		});
		expect(controller.enable).toHaveBeenCalledWith('codex');
	});

	it('retires the capability for malformed commands or transferred ports', () => {
		const malformedPort = new FakeMessagePort();
		const malformedBroker = new DesktopAgentIntegrationManagementBroker(
			malformedPort.asMessagePortMain(),
			createController(),
		);
		malformedBroker.start();
		malformedPort.emit({ type: 'untrusted-command' });
		expect(malformedPort.close).toHaveBeenCalledOnce();

		const transferredPort = new FakeMessagePort();
		const transferredBroker = new DesktopAgentIntegrationManagementBroker(
			transferredPort.asMessagePortMain(),
			createController(),
		);
		transferredBroker.start();
		transferredPort.emit(
			{
				requestId: 'agent-integration:request:3',
				type: 'desktop-agent-integration-refresh-requested',
			},
			[{}],
		);
		expect(transferredPort.close).toHaveBeenCalledOnce();
	});

	it('publishes a launch identity once without waiting for a renderer request', () => {
		const port = new FakeMessagePort();
		const controller = createController();
		const broker = new DesktopAgentIntegrationManagementBroker(
			port.asMessagePortMain(),
			controller,
		);
		broker.start();

		controller.publishLaunch('claude-code', 2);

		const integration = snapshot.integrations[0];
		expect(port.messages).toEqual([
			{
				integration,
				integrationId: 'claude-code',
				revision: 2,
				type: 'desktop-agent-integration-launch-observed',
			},
		]);
		broker.dispose();
		controller.publishLaunch('codex', 3);
		expect(port.messages).toHaveLength(1);
	});
});

function createController(): DesktopAgentIntegrationManagementBrokerController & {
	readonly enable: ReturnType<typeof vi.fn>;
	publishLaunch(
		integrationId: (typeof DESKTOP_AGENT_INTEGRATION_IDS)[number],
		revision: number,
	): void;
} {
	const launchListeners = new Set<
		Parameters<DesktopAgentIntegrationManagementBrokerController['subscribeLaunches']>[0]
	>();
	return {
		disable: vi.fn<DesktopAgentIntegrationManagementBrokerController['disable']>(async () => ({
			kind: 'completed',
			snapshot,
		})),
		enable: vi.fn<DesktopAgentIntegrationManagementBrokerController['enable']>(async () => ({
			kind: 'completed',
			snapshot,
		})),
		getSnapshot: () => snapshot,
		refresh: vi.fn<DesktopAgentIntegrationManagementBrokerController['refresh']>(async () => ({
			kind: 'completed',
			snapshot,
		})),
		publishLaunch: (integrationId, revision) => {
			const integration = snapshot.integrations.find(
				(candidate) => candidate.integrationId === integrationId,
			);
			if (integration === undefined) throw new Error('Missing test integration.');
			for (const listener of launchListeners) listener(integration, revision);
		},
		subscribeLaunches: (listener) => {
			launchListeners.add(listener);
			return () => launchListeners.delete(listener);
		},
	};
}

class FakeMessagePort {
	readonly #closeListeners = new Set<() => void>();
	readonly close = vi.fn<() => void>();
	readonly messages: unknown[] = [];
	readonly #messageListeners = new Set<(event: MessageEvent) => void>();

	asMessagePortMain(): MessagePortMain {
		return this as unknown as MessagePortMain;
	}

	emit(data: unknown, ports: readonly unknown[] = []): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports } as unknown as MessageEvent);
		}
	}

	off(event: string, listener: (...commandArguments: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.delete(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.delete(listener as unknown as () => void);
		}
	}

	on(event: string, listener: (...commandArguments: never[]) => void): void {
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
