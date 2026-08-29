import { DesktopSshProjectEnrollmentMainEventSchema } from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopSshProjectEnrollmentBroker,
	type DesktopSshProjectEnrollmentBrokerRuntime,
} from '../DesktopSshProjectEnrollmentBroker';

describe('Desktop SSH Project enrollment broker', () => {
	it('correlates progress and one transient prompt before publishing canonical admission', async () => {
		const port = new FakeMessagePort();
		let submittedResponse: unknown;
		const runtime = createRuntime({
			enroll: async (_input, operation) => {
				operation.progress('resolving-target');
				submittedResponse = await operation.interact({
					generation: 2,
					handle: 'ssh-prompt-test',
					kind: 'none',
					prompt: 'Password:',
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
		expect(DesktopSshProjectEnrollmentMainEventSchema.parse(port.messages[2])).toMatchObject({
			requestId: 'ssh-project:1',
			result: { canonicalPath: '/srv/orchbee', kind: 'created' },
			type: 'desktop-ssh-project-enrollment-settled',
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
});

function enrollmentRequest(requestId: string): unknown {
	return {
		input: {
			configurationFilePath: null,
			destination: 'build-host',
			name: 'Build machine',
			projectPath: '/srv/orchbee',
		},
		requestId,
		type: 'desktop-ssh-project-enrollment-requested',
	};
}

function createRuntime(
	overrides: Partial<DesktopSshProjectEnrollmentBrokerRuntime>,
): DesktopSshProjectEnrollmentBrokerRuntime {
	return {
		connectProject: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['connectProject']>(
			overrides.connectProject ?? (async () => ({ kind: 'connected', projectId: 'project-1' })),
		),
		disconnectProject: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['disconnectProject']>(
			overrides.disconnectProject ??
				(async () => ({ kind: 'disconnected', projectId: 'project-1' })),
		),
		enroll: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['enroll']>(
			overrides.enroll ?? (async () => ({ kind: 'cancelled' })),
		),
		statuses: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['statuses']>(
			overrides.statuses ?? (() => []),
		),
		subscribeStatuses: vi.fn<DesktopSshProjectEnrollmentBrokerRuntime['subscribeStatuses']>(
			overrides.subscribeStatuses ?? (() => () => undefined),
		),
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
