import {
	DesktopSshEnvironmentCatalogSchema,
	DesktopSshEnvironmentIdSchema,
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
		let resolvedDirectory: unknown;
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
							revision: 3,
							snapshot: {
								canonicalPath: '/home/builder',
								directories: [{ name: 'orchbee' }],
								navigationFailure: null,
								parentPath: '/home',
								truncated: false,
							},
							type: 'desktop-ssh-project-directory-selection-requested',
						}),
					);
					return;
				case 'desktop-ssh-project-directory-selection-resolved':
					resolvedDirectory = command;
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							requestId: command.requestId,
							result: {
								canonicalPath: '/home/builder',
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
		const selectDirectory = vi.fn<DesktopSshProjectOperationObserver['selectDirectory']>(
			async () => ({ kind: 'selected' }),
		);

		await expect(
			client.enroll(
				enrollmentInput(),
				{ interact, progress, selectDirectory },
				new AbortController().signal,
			),
		).resolves.toEqual({
			canonicalPath: '/home/builder',
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
		expect(selectDirectory).toHaveBeenCalledWith({
			canonicalPath: '/home/builder',
			directories: [{ name: 'orchbee' }],
			navigationFailure: null,
			parentPath: '/home',
			truncated: false,
		});
		expect(resolvedDirectory).toMatchObject({
			requestId: 'request:3',
			result: { kind: 'selected' },
			revision: 3,
		});
		expect(client.getStatuses()).toEqual([
			{ blockedReason: null, projectId: 'project-remote', state: 'disconnected' },
		]);
		expect(statusListener).toHaveBeenCalled();
		expect(commands).toHaveLength(5);
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
			{
				interact: async () => ({ kind: 'cancelled' }),
				selectDirectory: async () => ({ kind: 'cancelled' }),
			},
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

	it('drops a prompt completion that races behind cancellation and admits re-entry after settlement', async () => {
		const channel = new MessageChannel();
		const commands: unknown[] = [];
		let enrollmentCount = 0;
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopSshProjectEnrollmentRendererCommandSchema.parse(event.data);
			commands.push(command);
			if (command.type === 'desktop-ssh-project-enrollment-requested') {
				enrollmentCount += 1;
				if (enrollmentCount === 1) {
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							generation: 1,
							handle: 'prompt-cancelled',
							kind: 'none',
							prompt: 'Password:',
							requestId: command.requestId,
							type: 'desktop-ssh-project-interaction-requested',
						}),
					);
				} else {
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							requestId: command.requestId,
							result: { kind: 'cancelled' },
							type: 'desktop-ssh-project-enrollment-settled',
						}),
					);
				}
				return;
			}
			if (command.type === 'desktop-ssh-project-enrollment-cancel-requested') {
				channel.port2.postMessage(
					DesktopSshProjectEnrollmentMainEventSchema.parse({
						requestId: command.requestId,
						result: { kind: 'cancelled' },
						type: 'desktop-ssh-project-enrollment-settled',
					}),
				);
			}
		});
		channel.port2.start();
		const client = new DesktopSshProjectEnrollmentClient(channel.port1, () => 'cancel-race');
		const interaction = createDeferred<{ readonly kind: 'cancelled' }>();
		const interact = vi.fn<DesktopSshProjectOperationObserver['interact']>(
			() => interaction.promise,
		);
		const abortController = new AbortController();
		const firstEnrollment = client.enroll(
			enrollmentInput(),
			{
				interact,
				selectDirectory: async () => ({ kind: 'cancelled' }),
			},
			abortController.signal,
		);
		await vi.waitFor(() => expect(interact).toHaveBeenCalledOnce());
		interaction.resolve({ kind: 'cancelled' });
		abortController.abort();

		await expect(firstEnrollment).resolves.toEqual({ kind: 'cancelled' });
		await vi.waitFor(() =>
			expect(commands).toContainEqual(
				expect.objectContaining({ type: 'desktop-ssh-project-enrollment-cancel-requested' }),
			),
		);
		expect(commands).not.toContainEqual(
			expect.objectContaining({ type: 'desktop-ssh-project-interaction-resolved' }),
		);
		await expect(
			client.enroll(
				enrollmentInput(),
				{
					interact: async () => ({ kind: 'cancelled' }),
					selectDirectory: async () => ({ kind: 'cancelled' }),
				},
				new AbortController().signal,
			),
		).resolves.toEqual({ kind: 'cancelled' });
		expect(enrollmentCount).toBe(2);

		client.dispose();
		channel.port2.close();
	});

	it('correlates Folder attachment with its exact Project and canonical Directory result', async () => {
		const channel = new MessageChannel();
		const commands: unknown[] = [];
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopSshProjectEnrollmentRendererCommandSchema.parse(event.data);
			commands.push(command);
			if (command.type === 'desktop-ssh-project-folder-attachment-requested') {
				channel.port2.postMessage(
					DesktopSshProjectEnrollmentMainEventSchema.parse({
						requestId: command.requestId,
						stage: 'committing-project-folder',
						type: 'desktop-ssh-project-folder-attachment-progressed',
					}),
				);
				channel.port2.postMessage(
					DesktopSshProjectEnrollmentMainEventSchema.parse({
						requestId: command.requestId,
						revision: 1,
						snapshot: {
							canonicalPath: '/srv',
							directories: [{ name: 'shared' }],
							navigationFailure: null,
							parentPath: '/',
							truncated: false,
						},
						type: 'desktop-ssh-project-directory-selection-requested',
					}),
				);
				return;
			}
			if (command.type === 'desktop-ssh-project-directory-selection-resolved') {
				channel.port2.postMessage(
					DesktopSshProjectEnrollmentMainEventSchema.parse({
						requestId: command.requestId,
						result: {
							canonicalPath: '/srv',
							directoryId: 'environment-directory:44444444444444444444444444444444',
							kind: 'attached',
							projectId: 'project-local',
						},
						type: 'desktop-ssh-project-folder-attachment-settled',
					}),
				);
			}
		});
		channel.port2.start();
		const client = new DesktopSshProjectEnrollmentClient(channel.port1, () => 'attachment');
		const progress = vi.fn<(stage: DesktopSshProjectEnrollmentProgressStage) => void>();

		await expect(
			client.attachFolder(
				'project-local',
				{ destination: 'builder@host' },
				{
					interact: async () => ({ kind: 'cancelled' }),
					progress,
					selectDirectory: async () => ({ kind: 'selected' }),
				},
				new AbortController().signal,
			),
		).resolves.toEqual({
			canonicalPath: '/srv',
			directoryId: 'environment-directory:44444444444444444444444444444444',
			kind: 'attached',
			projectId: 'project-local',
		});
		expect(commands).toContainEqual(
			expect.objectContaining({
				input: { destination: 'builder@host' },
				projectId: 'project-local',
				type: 'desktop-ssh-project-folder-attachment-requested',
			}),
		);
		expect(progress).toHaveBeenCalledWith('committing-project-folder');
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
				{
					interact: () => new Promise(() => undefined),
					selectDirectory: async () => ({ kind: 'cancelled' }),
				},
				new AbortController().signal,
			),
		).resolves.toEqual({ kind: 'indeterminate', reason: 'control-unavailable' });
		expect(close).toHaveBeenCalledOnce();
		channel.port2.close();
	});

	it('caches discovery and correlates reusable Environment add and connection operations', async () => {
		const channel = new MessageChannel();
		const commands: unknown[] = [];
		const environmentId = DesktopSshEnvironmentIdSchema.parse(`environment:${'a'.repeat(32)}`);
		const catalog = DesktopSshEnvironmentCatalogSchema.parse({
			discoveryIssue: null,
			managed: [],
			registryIssue: null,
			suggestions: [{ destination: 'gpu-lab' }],
		});
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopSshProjectEnrollmentRendererCommandSchema.parse(event.data);
			commands.push(command);
			switch (command.type) {
				case 'desktop-ssh-environment-catalog-requested':
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							catalog,
							requestId: command.requestId,
							type: 'desktop-ssh-environment-catalog-published',
						}),
					);
					return;
				case 'desktop-ssh-environment-add-requested':
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							requestId: command.requestId,
							result: { environmentId, kind: 'added' },
							type: 'desktop-ssh-environment-add-settled',
						}),
					);
					return;
				case 'desktop-ssh-environment-connect-requested':
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							generation: 4,
							handle: 'ssh-environment-prompt',
							kind: 'none',
							prompt: 'Password:',
							requestId: command.requestId,
							type: 'desktop-ssh-project-interaction-requested',
						}),
					);
					return;
				case 'desktop-ssh-project-interaction-resolved':
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							requestId: command.requestId,
							result: { environmentId, kind: 'connected' },
							type: 'desktop-ssh-environment-connection-settled',
						}),
					);
					return;
				case 'desktop-ssh-environment-disconnect-requested':
					channel.port2.postMessage(
						DesktopSshProjectEnrollmentMainEventSchema.parse({
							requestId: command.requestId,
							result: { environmentId, kind: 'disconnected' },
							type: 'desktop-ssh-environment-connection-settled',
						}),
					);
					return;
				default:
					return;
			}
		});
		channel.port2.start();
		const client = new DesktopSshProjectEnrollmentClient(channel.port1, () => 'management');
		const catalogListener = vi.fn<() => void>();
		client.subscribeEnvironmentCatalog(catalogListener);
		await vi.waitFor(() => expect(client.getEnvironmentCatalog()).toEqual(catalog));
		expect(catalogListener).toHaveBeenCalledOnce();
		expect(commands).toContainEqual(
			expect.objectContaining({
				refreshDiscovery: false,
				type: 'desktop-ssh-environment-catalog-requested',
			}),
		);

		await expect(
			client.addEnvironment({ destination: 'gpu-lab' }, new AbortController().signal),
		).resolves.toEqual({ environmentId, kind: 'added' });
		const interact = vi.fn<DesktopSshProjectOperationObserver['interact']>(async () => ({
			kind: 'submitted' as const,
			response: 'one-use secret',
		}));
		await expect(
			client.connectEnvironment(environmentId, { interact }, new AbortController().signal),
		).resolves.toEqual({ environmentId, kind: 'connected' });
		expect(interact).toHaveBeenCalledWith({
			generation: 4,
			handle: 'ssh-environment-prompt',
			kind: 'none',
			prompt: 'Password:',
		});
		await expect(
			client.disconnectEnvironment(environmentId, new AbortController().signal),
		).resolves.toEqual({ environmentId, kind: 'disconnected' });
		client.dispose();
		channel.port2.close();
	});
});

function enrollmentInput() {
	return {
		destination: 'build-host',
	};
}

function createDeferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}
