import {
	DesktopSshEnvironmentCatalogSchema,
	DesktopSshEnvironmentIdSchema,
	DesktopSshProjectEnrollmentMainEventSchema,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopSshProjectEnrollmentBroker,
	type DesktopSshProjectEnrollmentBrokerRuntime,
} from '../DesktopSshProjectEnrollmentBroker';

describe('Desktop SSH Project enrollment broker', () => {
	it('correlates a prompt and directory revision before publishing canonical admission', async () => {
		const port = new FakeMessagePort();
		let submittedResponse: unknown;
		let selectedDirectory: unknown;
		const runtime = createRuntime({
			enroll: async (_input, operation) => {
				operation.progress('resolving-target');
				submittedResponse = await operation.interact({
					generation: 2,
					handle: 'ssh-prompt-test',
					kind: 'none',
					prompt: 'Password:',
				});
				selectedDirectory = await operation.selectDirectory({
					canonicalPath: '/home/builder',
					directories: [{ name: 'src' }],
					navigationFailure: null,
					parentPath: '/home',
					truncated: false,
				});
				return {
					canonicalPath: '/srv/orchbee',
					kind: 'created',
					projectId: 'project-1',
					workspaceId: 'workspace-1',
				};
			},
		});
		const broker = new DesktopSshProjectEnrollmentBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit(enrollmentRequest('ssh-project:1'));
		await vi.waitFor(() => expect(port.messages).toHaveLength(2));

		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[0])).toEqual({
			requestId: 'ssh-project:1',
			stage: 'resolving-target',
			type: 'desktop-ssh-project-enrollment-progressed',
		});
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[1])).toEqual({
			generation: 2,
			handle: 'ssh-prompt-test',
			kind: 'none',
			prompt: 'Password:',
			requestId: 'ssh-project:1',
			type: 'desktop-ssh-project-interaction-requested',
		});
		port.emit({
			generation: 2,
			handle: 'ssh-prompt-test',
			requestId: 'ssh-project:1',
			result: { kind: 'submitted', response: 'one-use secret' },
			type: 'desktop-ssh-project-interaction-resolved',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(3));
		expect(submittedResponse).toEqual({ kind: 'submitted', response: 'one-use secret' });
		const selectionRequest = DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[2]);
		expect(selectionRequest).toMatchObject({
			requestId: 'ssh-project:1',
			revision: 1,
			snapshot: { canonicalPath: '/home/builder' },
			type: 'desktop-ssh-project-directory-selection-requested',
		});
		port.emit({
			requestId: 'ssh-project:1',
			result: { kind: 'selected' },
			revision: 1,
			type: 'desktop-ssh-project-directory-selection-resolved',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(4));
		expect(selectedDirectory).toEqual({ kind: 'selected' });
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[3])).toMatchObject({
			requestId: 'ssh-project:1',
			result: { canonicalPath: '/srv/orchbee', kind: 'created' },
			type: 'desktop-ssh-project-enrollment-settled',
		});
	});

	it('retires the port when a response uses a stale directory revision', async () => {
		const port = new FakeMessagePort();
		const runtime = createRuntime({
			enroll: async (_input, operation) => {
				await operation.selectDirectory({
					canonicalPath: '/home/builder',
					directories: [],
					navigationFailure: null,
					parentPath: '/home',
					truncated: false,
				});
				return { kind: 'cancelled' };
			},
		});
		const broker = new DesktopSshProjectEnrollmentBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit(enrollmentRequest('ssh-project:directory'));
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));
		port.emit({
			requestId: 'ssh-project:directory',
			result: { kind: 'selected' },
			revision: 2,
			type: 'desktop-ssh-project-directory-selection-resolved',
		});
		expect(port.close).toHaveBeenCalledOnce();
	});

	it('correlates the same directory interaction for existing-Project Folder attachment', async () => {
		const port = new FakeMessagePort();
		const runtime = createRuntime({
			attachFolder: async (projectId, _input, operation) => {
				operation.progress('committing-project-folder');
				await operation.selectDirectory({
					canonicalPath: '/srv/orchbee',
					directories: [],
					navigationFailure: null,
					parentPath: '/srv',
					truncated: false,
				});
				return {
					canonicalPath: '/srv/orchbee',
					directoryId: 'environment-directory:11111111111111111111111111111111',
					kind: 'attached',
					projectId,
				};
			},
		});
		const broker = new DesktopSshProjectEnrollmentBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit({
			input: { destination: 'build-host' },
			projectId: 'project-local',
			requestId: 'ssh-project-folder:1',
			type: 'desktop-ssh-project-folder-attachment-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(2));
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[0])).toEqual({
			requestId: 'ssh-project-folder:1',
			stage: 'committing-project-folder',
			type: 'desktop-ssh-project-folder-attachment-progressed',
		});
		const selection = DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[1]);
		expect(selection).toMatchObject({
			requestId: 'ssh-project-folder:1',
			revision: 1,
			type: 'desktop-ssh-project-directory-selection-requested',
		});
		port.emit({
			requestId: 'ssh-project-folder:1',
			result: { kind: 'selected' },
			revision: 1,
			type: 'desktop-ssh-project-directory-selection-resolved',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(3));
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[2])).toMatchObject({
			requestId: 'ssh-project-folder:1',
			result: { directoryId: expect.any(String), kind: 'attached', projectId: 'project-local' },
			type: 'desktop-ssh-project-folder-attachment-settled',
		});
	});

	it('deduplicates an exact replay, rejects concurrent work, and revokes on port loss', async () => {
		const port = new FakeMessagePort();
		let signal: AbortSignal | undefined;
		let settle: (() => void) | undefined;
		const runtime = createRuntime({
			enroll: (_input, _operation, operationSignal) =>
				new Promise((resolve) => {
					signal = operationSignal;
					settle = () => resolve({ kind: 'cancelled' });
				}),
		});
		const broker = new DesktopSshProjectEnrollmentBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit(enrollmentRequest('ssh-project:1'));
		port.emit(enrollmentRequest('ssh-project:1'));
		port.emit({
			projectId: 'project-1',
			requestId: 'ssh-project:2',
			type: 'desktop-ssh-project-connect-requested',
		});

		expect(runtime.enroll).toHaveBeenCalledOnce();
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[0])).toMatchObject({
			requestId: 'ssh-project:2',
			result: { kind: 'failed', reason: 'busy' },
			type: 'desktop-ssh-project-connection-settled',
		});
		port.emitClose();
		expect(signal?.aborted).toBe(true);
		settle?.();
		await Promise.resolve();
		expect(port.close).not.toHaveBeenCalled();
	});

	it('retires the port when a response does not own the exact prompt handle', async () => {
		const port = new FakeMessagePort();
		const runtime = createRuntime({
			enroll: async (_input, operation) => {
				await operation.interact({
					generation: 1,
					handle: 'ssh-prompt-owned',
					kind: 'confirm',
					prompt: 'Trust host?',
				});
				return { kind: 'cancelled' };
			},
		});
		const broker = new DesktopSshProjectEnrollmentBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit(enrollmentRequest('ssh-project:1'));
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));
		port.emit({
			generation: 1,
			handle: 'ssh-prompt-stolen',
			requestId: 'ssh-project:1',
			result: { kind: 'submitted', response: 'yes' },
			type: 'desktop-ssh-project-interaction-resolved',
		});
		expect(port.close).toHaveBeenCalledOnce();
	});

	it('publishes discovery and correlates explicit reusable Environment operations', async () => {
		const port = new FakeMessagePort();
		const environmentId = DesktopSshEnvironmentIdSchema.parse(`environment:${'a'.repeat(32)}`);
		const catalog = DesktopSshEnvironmentCatalogSchema.parse({
			discoveryIssue: null,
			managed: [],
			registryIssue: null,
			suggestions: [{ destination: 'gpu-lab' }],
		});
		const runtime = createRuntime({
			addEnvironment: async () => ({ environmentId, kind: 'added' }),
			connectEnvironment: async (candidate, operation) => {
				await operation.interact({
					generation: 3,
					handle: 'ssh-environment-prompt',
					kind: 'none',
					prompt: 'Password:',
				});
				expect(candidate).toBe(environmentId);
				return { environmentId, kind: 'connected' };
			},
			disconnectEnvironment: async (candidate) => {
				expect(candidate).toBe(environmentId);
				return { environmentId, kind: 'disconnected' };
			},
			environmentCatalog: async () => catalog,
		});
		const broker = new DesktopSshProjectEnrollmentBroker(port.asMessagePortMain(), runtime);
		broker.start();

		port.emit({
			refreshDiscovery: true,
			requestId: 'ssh-catalog:1',
			type: 'desktop-ssh-environment-catalog-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));
		expect(runtime.environmentCatalog).toHaveBeenCalledWith(true);
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[0])).toEqual({
			catalog,
			requestId: 'ssh-catalog:1',
			type: 'desktop-ssh-environment-catalog-published',
		});

		port.emit({
			input: { destination: 'gpu-lab' },
			requestId: 'ssh-add:1',
			type: 'desktop-ssh-environment-add-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(2));
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[1])).toMatchObject({
			requestId: 'ssh-add:1',
			result: { environmentId, kind: 'added' },
			type: 'desktop-ssh-environment-add-settled',
		});

		port.emit({
			environmentId,
			requestId: 'ssh-environment-connect:1',
			type: 'desktop-ssh-environment-connect-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(3));
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[2])).toMatchObject({
			handle: 'ssh-environment-prompt',
			requestId: 'ssh-environment-connect:1',
			type: 'desktop-ssh-project-interaction-requested',
		});
		port.emit({
			generation: 3,
			handle: 'ssh-environment-prompt',
			requestId: 'ssh-environment-connect:1',
			result: { kind: 'submitted', response: 'one-use secret' },
			type: 'desktop-ssh-project-interaction-resolved',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(4));
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[3])).toMatchObject({
			requestId: 'ssh-environment-connect:1',
			result: { environmentId, kind: 'connected' },
			type: 'desktop-ssh-environment-connection-settled',
		});

		port.emit({
			environmentId,
			requestId: 'ssh-environment-disconnect:1',
			type: 'desktop-ssh-environment-disconnect-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(5));
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[4])).toMatchObject({
			requestId: 'ssh-environment-disconnect:1',
			result: { environmentId, kind: 'disconnected' },
			type: 'desktop-ssh-environment-connection-settled',
		});
		broker.dispose();
	});
});

function enrollmentRequest(requestId: string): unknown {
	return {
		input: {
			destination: 'build-host',
		},
		requestId,
		type: 'desktop-ssh-project-enrollment-requested',
	};
}

function createRuntime(
	overrides: Partial<DesktopSshProjectEnrollmentBrokerRuntime>,
): DesktopSshProjectEnrollmentBrokerRuntime {
	return {
		addEnvironment: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['addEnvironment']>(
			overrides.addEnvironment ?? (async () => ({ kind: 'cancelled' })),
		),
		attachFolder: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['attachFolder']>(
			overrides.attachFolder ?? (async () => ({ kind: 'cancelled' })),
		),
		connectProject: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['connectProject']>(
			overrides.connectProject ?? (async () => ({ kind: 'connected', projectId: 'project-1' })),
		),
		connectEnvironment: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['connectEnvironment']>(
			overrides.connectEnvironment ?? (async () => ({ kind: 'cancelled' })),
		),
		disconnectEnvironment: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['disconnectEnvironment']>(
			overrides.disconnectEnvironment ?? (async () => ({ kind: 'cancelled' })),
		),
		disconnectProject: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['disconnectProject']>(
			overrides.disconnectProject ??
				(async () => ({ kind: 'disconnected', projectId: 'project-1' })),
		),
		enroll: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['enroll']>(
			overrides.enroll ?? (async () => ({ kind: 'cancelled' })),
		),
		environmentCatalog: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['environmentCatalog']>(
			overrides.environmentCatalog ??
				(async () => ({
					discoveryIssue: null,
					managed: [],
					registryIssue: null,
					suggestions: [],
				})),
		),
		statuses: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['statuses']>(
			overrides.statuses ?? (() => []),
		),
		subscribeStatuses: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['subscribeStatuses']>(
			overrides.subscribeStatuses ?? (() => () => undefined),
		),
		subscribeEnvironmentCatalog: vi.fn<
			DesktopSshProjectEnrollmentBrokerRuntime['subscribeEnvironmentCatalog']
		>(overrides.subscribeEnvironmentCatalog ?? (() => () => undefined)),
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

	emit(data: unknown): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports: [] } as unknown as MessageEvent);
		}
	}

	emitClose(): void {
		for (const listener of this.#closeListeners) listener();
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
