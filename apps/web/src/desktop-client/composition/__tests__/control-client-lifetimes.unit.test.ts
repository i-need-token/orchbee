import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopAgentIntegrationManagementMainEventSchema,
	DesktopAgentIntegrationManagementRendererCommandSchema,
	DesktopTerminalDefaultSettingsMainEventSchema,
	DesktopTerminalDefaultSettingsRendererCommandSchema,
	DesktopTerminalLaunchProfileMainEventSchema,
	DesktopTerminalLaunchProfileRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { DesktopAgentIntegrationManagementClient } from '../../agent-integration-management';
import { DesktopTerminalDefaultSettingsClient } from '../../terminal-default-settings';
import { DesktopTerminalLaunchProfileClient } from '../../terminal-launch-profiles';

interface ControlClient {
	initialize(): Promise<{ readonly kind: string }>;
	getSnapshot(): { readonly pending: boolean; readonly failureCode: string | null };
	subscribe(listener: () => void): () => void;
	dispose(): void;
}

interface ControlFixture {
	readonly client: ControlClient;
	readonly operate: () => Promise<{ readonly kind: string }>;
	readonly reply: (index: number) => void;
	readonly port: ControlledPort;
}

const cleanup: (() => void)[] = [];
afterEach(() => {
	while (cleanup.length !== 0) cleanup.pop()?.();
});

function observe(client: ControlClient, listener: () => void): () => void {
	const remove = client.subscribe(listener);
	cleanup.push(remove);
	return remove;
}

function fixture(owner: 'profiles' | 'defaults' | 'integrations'): ControlFixture {
	const port = new ControlledPort();
	let control: Omit<ControlFixture, 'port'>;
	if (owner === 'profiles') {
		const client = new DesktopTerminalLaunchProfileClient(port.asMessagePort(), () => 'request');
		control = {
			client,
			operate: () => client.reset(),
			reply: (index) => {
				const request = DesktopTerminalLaunchProfileRendererCommandSchema.parse(
					port.messages[index],
				);
				port.emit(
					DesktopTerminalLaunchProfileMainEventSchema.parse({
						type: 'desktop-terminal-launch-profile-responded',
						requestId: request.requestId,
						result: {
							kind: 'completed',
							snapshot: {
								capabilities: { loginShell: true },
								persistenceIssue: null,
								source: 'built-in',
								catalog: {
									version: 1,
									defaultProfileId: 'system-default',
									profiles: [
										{
											id: 'system-default',
											name: null,
											command: { kind: 'system-shell', login: false },
										},
									],
								},
							},
						},
					}),
				);
			},
		};
	} else if (owner === 'defaults') {
		const client = new DesktopTerminalDefaultSettingsClient(port.asMessagePort(), () => 'request');
		control = {
			client,
			operate: () => client.update({ scrollbackBytes: 25_000_000 }),
			reply: (index) => {
				const request = DesktopTerminalDefaultSettingsRendererCommandSchema.parse(
					port.messages[index],
				);
				port.emit(
					DesktopTerminalDefaultSettingsMainEventSchema.parse({
						type: 'desktop-terminal-default-settings-responded',
						requestId: request.requestId,
						result: {
							kind: 'completed',
							snapshot: {
								configuration: { scrollbackBytes: index === 0 ? 10_000_000 : 25_000_000 },
								persistenceIssue: null,
								source: 'stored',
							},
						},
					}),
				);
			},
		};
	} else {
		const client = new DesktopAgentIntegrationManagementClient(
			port.asMessagePort(),
			() => 'request',
		);
		control = {
			client,
			operate: () => client.enable('codex'),
			reply: (index) => {
				const request = DesktopAgentIntegrationManagementRendererCommandSchema.parse(
					port.messages[index],
				);
				const codexState = index === 0 ? 'not-installed' : 'enabled';
				port.emit(
					DesktopAgentIntegrationManagementMainEventSchema.parse({
						type: 'desktop-agent-integration-responded',
						requestId: request.requestId,
						result: {
							kind: 'completed',
							snapshot: {
								integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
									integrationId,
									availability: integrationId === 'codex' ? 'available' : 'not-found',
									issue: null,
									managementMode: 'managed-local',
									state: integrationId === 'codex' ? codexState : 'unknown',
								})),
								launchedIntegrationIds: [],
								revision: index + 1,
							},
						},
					}),
				);
			},
		};
	}
	cleanup.push(() => control.client.dispose());
	return { ...control, port };
}

async function initialize(test: ControlFixture): Promise<void> {
	const ready = test.client.initialize();
	test.reply(0);
	await expect(ready).resolves.toEqual({ kind: 'ready' });
}

describe.each(['profiles', 'defaults', 'integrations'] as const)(
	'%s control client lifetime',
	(owner) => {
		it.each(['admission', 'reply', 'retirement'] as const)(
			'settles the owned request when an observer throws during %s',
			async (phase) => {
				const test = fixture(owner);
				await initialize(test);
				const failure = new Error('observer failure');
				observe(test.client, () => {
					const snapshot = test.client.getSnapshot();
					if (
						(phase === 'admission' && snapshot.pending) ||
						(phase === 'reply' && !snapshot.pending && snapshot.failureCode === null) ||
						(phase === 'retirement' && snapshot.failureCode === 'control-unavailable')
					)
						throw failure;
				});
				const healthy = vi.fn<() => void>();
				observe(test.client, healthy);
				const pending = test.operate();
				// Record an unexpected rejection immediately so the negative trace cannot leak it.
				const rejected = vi.fn<(reason: unknown) => void>();
				void pending.catch(rejected);
				expect(test.port.messages).toHaveLength(2);
				expect(healthy).toHaveBeenCalledOnce();
				const finish = phase === 'retirement' ? () => test.client.dispose() : () => test.reply(1);
				expect(finish).not.toThrow();
				await expect(pending).resolves.toMatchObject({
					kind: phase === 'retirement' ? 'unavailable' : 'completed',
				});
				expect(healthy).toHaveBeenCalledTimes(2);
				expect(rejected).not.toHaveBeenCalled();
			},
		);

		it('visits a callback once when it resubscribes during its current notification', async () => {
			const test = fixture(owner);
			await initialize(test);
			let calls = 0;
			const listener = (): void => {
				calls += 1;
				if (calls === 1) {
					remove();
					remove = observe(test.client, listener);
				}
			};
			let remove = observe(test.client, listener);
			const pending = test.operate();
			expect(calls).toBe(1);
			test.reply(1);
			await expect(pending).resolves.toMatchObject({ kind: 'completed' });
			expect(calls).toBe(2);
		});

		it('skips a callback removed before its turn in the captured notification', async () => {
			const test = fixture(owner);
			await initialize(test);
			observe(test.client, () => remove());
			const listener = vi.fn<() => void>();
			const remove = observe(test.client, listener);
			const pending = test.operate();
			expect(listener).not.toHaveBeenCalled();
			test.reply(1);
			await expect(pending).resolves.toMatchObject({ kind: 'completed' });
			expect(listener).not.toHaveBeenCalled();
		});

		it('retires during pending publication without posting or repeating terminal notification', async () => {
			const test = fixture(owner);
			await initialize(test);
			observe(test.client, () => {
				if (test.client.getSnapshot().pending) test.client.dispose();
			});
			const terminal = vi.fn<() => void>();
			observe(test.client, terminal);
			await expect(test.operate()).resolves.toEqual({ kind: 'unavailable' });
			expect(test.port.messages).toHaveLength(1);
			expect(test.port.close).toHaveBeenCalledOnce();
			expect(terminal).toHaveBeenCalledOnce();
			expect(test.client.getSnapshot()).toMatchObject({
				pending: false,
				failureCode: 'control-unavailable',
			});
		});
	},
);

/** Controls callback order and injected observer faults; it does not simulate native transport. */
class ControlledPort {
	readonly close = vi.fn<() => void>();
	readonly messages: unknown[] = [];
	readonly #listeners = new Set<EventListenerOrEventListenerObject>();
	readonly postMessage = vi.fn<(message: unknown) => void>((message) => {
		this.messages.push(message);
	});
	addEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
		if (type === 'message') this.#listeners.add(listener);
	}
	removeEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
		if (type === 'message') this.#listeners.delete(listener);
	}
	emit(data: unknown): void {
		const event = new MessageEvent<unknown>('message', { data });
		for (const listener of new Set(this.#listeners)) {
			if (!this.#listeners.has(listener)) continue;
			if (typeof listener === 'function') listener(event);
			else listener.handleEvent(event);
		}
	}
	asMessagePort(): MessagePort {
		return this as unknown as MessagePort;
	}
	start(): void {}
}
