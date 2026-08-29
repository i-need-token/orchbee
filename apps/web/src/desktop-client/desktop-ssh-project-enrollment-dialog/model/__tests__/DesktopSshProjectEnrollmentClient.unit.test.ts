import {
	DesktopSshProjectEnrollmentMainEventSchema,
	DesktopSshProjectEnrollmentRendererCommandSchema,
	type DesktopSshProjectEnrollmentProgressStage,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopSshProjectEnrollmentClient,
	type DesktopSshProjectOperationObserver,
} from '../DesktopSshProjectEnrollmentClient';

describe('DesktopSshProjectEnrollmentClient', () => {
	it('correlates status, progress, one transient prompt, and the canonical admission result', async () => {
		const channel = new MessageChannel();
		const commands: unknown[] = [];
		let resolvedInteraction: unknown;
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopSshProjectEnrollmentRendererCommandSchema.parse(event.data);
			commands.push(command);
			switch (command.type) {
				case 'desktop-ssh-project-status-requested':
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							requestId: command.requestId,
							statuses: [
								{ blockedReason: null, projectId: 'project-remote', state: 'disconnected' },
							],
							type: 'desktop-ssh-project-status-published',
						}),
					);
					return;
				case 'desktop-ssh-project-enrollment-requested':
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							requestId: command.requestId,
							stage: 'connecting',
							type: 'desktop-ssh-project-enrollment-progressed',
						}),
					);
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							generation: 7,
							handle: 'prompt-7',
							kind: 'none',
							prompt: 'Enter passphrase:',
							requestId: command.requestId,
							type: 'desktop-ssh-project-interaction-requested',
						}),
					);
					return;
				case 'desktop-ssh-project-interaction-resolved':
					resolvedInteraction = command;
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							requestId: command.requestId,
							result: {
								canonicalPath: '/srv/orchbee',
								kind: 'created',
								projectId: 'project-remote',
								workspaceId: 'workspace-primary',
							},
							type: 'desktop-ssh-project-enrollment-settled',
						}),
					);
					return;
				default:
					return;
			}
		});
		channel.port2.start();
		const client = new DesktopSshProjectEnrollmentClient(channel.port1, () => 'request');
		const statusListener = vi.fn<() => void>();
		client.subscribeStatuses(statusListener);
		const progress = vi.fn<(stage: DesktopSshProjectEnrollmentProgressStage) => void>();
		const interact = vi.fn<DesktopSshProjectOperationObserver['interact']>(async () => ({
			kind: 'submitted' as const,
			response: 'one-use secret',
		}));

		await expect(
			client.enroll(enrollmentInput(), { interact, progress }, new AbortController().signal),
		).resolves.toEqual({
			canonicalPath: '/srv/orchbee',
			kind: 'created',
			projectId: 'project-remote',
			workspaceId: 'workspace-primary',
		});
		expect(progress).toHaveBeenCalledWith('connecting');
		expect(interact).toHaveBeenCalledWith({
			generation: 7,
			handle: 'prompt-7',
			kind: 'none',
			prompt: 'Enter passphrase:',
		});
		expect(resolvedInteraction).toMatchObject({
			generation: 7,
			handle: 'prompt-7',
			result: { kind: 'submitted', response: 'one-use secret' },
		});
		expect(client.getStatuses()).toEqual([
			{ blockedReason: null, projectId: 'project-remote', state: 'disconnected' },
		]);
		expect(statusListener).toHaveBeenCalled();
		expect(commands).toHaveLength(3);
		client.dispose();
		channel.port2.close();
	});

	it('sends cancellation, waits for Main settlement, and rejects overlapping work', async () => {
		const channel = new MessageChannel();
		let enrollmentRequestId: string | undefined;
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopSshProjectEnrollmentRendererCommandSchema.parse(event.data);
			if (command.type === 'desktop-ssh-project-enrollment-requested') {
				enrollmentRequestId = command.requestId;
			}
			if (command.type !== 'desktop-ssh-project-enrollment-cancel-requested') return;
			channel.port2.postMessage(
				DesktopSshProjectEnrollmentMainEventSchema.parse({
					requestId: command.requestId,
					result: { kind: 'cancelled' },
					type: 'desktop-ssh-project-enrollment-settled',
				}),
			);
		});
		channel.port2.start();
		const client = new DesktopSshProjectEnrollmentClient(channel.port1, () => 'request');
		const abortController = new AbortController();
		const enrollment = client.enroll(
			enrollmentInput(),
			{ interact: async () => ({ kind: 'cancelled' }) },
			abortController.signal,
		);
		await vi.waitFor(() => expect(enrollmentRequestId).toBeDefined());

		await expect(
			client.connect(
				'project-remote',
				{ interact: async () => ({ kind: 'cancelled' }) },
				new AbortController().signal,
			),
		).resolves.toEqual({ kind: 'failed', reason: 'busy' });
		abortController.abort();
		await expect(enrollment).resolves.toEqual({ kind: 'cancelled' });

		client.dispose();
		channel.port2.close();
	});

	it('retires the authority when Main issues a second concurrent prompt', async () => {
		const channel = new MessageChannel();
		const close = vi.spyOn(channel.port1, 'close');
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopSshProjectEnrollmentRendererCommandSchema.parse(event.data);
			if (command.type !== 'desktop-ssh-project-enrollment-requested') return;
			for (const handle of ['prompt-1', 'prompt-2']) {
				channel.port2.postMessage(
					DesktopSshProjectEnrollmentMainEventSchema.parse({
						generation: 1,
						handle,
						kind: 'none',
						prompt: 'Password:',
						requestId: command.requestId,
						type: 'desktop-ssh-project-interaction-requested',
					}),
				);
			}
		});
		channel.port2.start();
		const client = new DesktopSshProjectEnrollmentClient(channel.port1, () => 'request');

		await expect(
			client.enroll(
				enrollmentInput(),
				{ interact: () => new Promise(() => undefined) },
				new AbortController().signal,
			),
		).resolves.toEqual({ kind: 'indeterminate', reason: 'control-unavailable' });
		expect(close).toHaveBeenCalledOnce();
		channel.port2.close();
	});
});

function enrollmentInput() {
	return {
		configurationFilePath: null,
		destination: 'build-host',
		name: 'Build machine',
		projectPath: '/srv/orchbee',
	};
}
