import type {
	DesktopSshProjectConnectionStatus,
	DesktopSshProjectEnrollmentResult,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopSshProjectEnrollmentController,
	type DesktopSshProjectEnrollmentControllerClient,
	type DesktopSshProjectEnrollmentControllerOptions,
} from '../DesktopSshProjectEnrollmentController';

describe('DesktopSshProjectEnrollmentController', () => {
	it('validates enrollment locally and keeps authentication responses outside dialog state', async () => {
		const responses: unknown[] = [];
		const client = createClient({
			enroll: async (_input, observer) => {
				observer.progress?.('connecting');
				responses.push(
					await observer.interact({
						generation: 4,
						handle: 'prompt-4',
						kind: 'none',
						prompt: 'Enter passphrase:',
					}),
				);
				return enrollmentCreated();
			},
		});
		const committed = vi.fn<DesktopSshProjectEnrollmentControllerOptions['onEnrollmentCommitted']>(
			async () => true,
		);
		const controller = new DesktopSshProjectEnrollmentController({
			client,
			onEnrollmentCommitted: committed,
		});

		controller.openEnrollment();
		expect(controller.getDialogSnapshot()).toMatchObject({ canSubmit: false, phase: 'editing' });
		controller.updateForm({
			configurationFilePath: ' /Users/me/.ssh/config ',
			destination: ' build-host ',
			name: ' Build machine ',
			projectPath: ' /srv/orchbee ',
		});
		expect(controller.getDialogSnapshot()).toMatchObject({ canSubmit: true, phase: 'editing' });
		controller.submit();
		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({
				phase: 'prompt',
				progress: 'connecting',
				prompt: { handle: 'prompt-4' },
			}),
		);
		expect(JSON.stringify(controller.getDialogSnapshot())).not.toContain('one-use secret');
		controller.resolvePrompt({ kind: 'submitted', response: 'one-use secret' });

		await vi.waitFor(() => expect(controller.getDialogSnapshot()).toEqual({ kind: 'closed' }));
		expect(client.enroll).toHaveBeenCalledWith(
			{
				configurationFilePath: '/Users/me/.ssh/config',
				destination: 'build-host',
				name: 'Build machine',
				projectPath: '/srv/orchbee',
			},
			expect.any(Object),
			expect.any(AbortSignal),
		);
		expect(responses).toEqual([{ kind: 'submitted', response: 'one-use secret' }]);
		expect(committed).toHaveBeenCalledWith(enrollmentCreated());
		controller.dispose();
	});

	it('closing an interaction revokes the prompt and aborts the in-flight operation', async () => {
		let operationSignal: AbortSignal | undefined;
		let interactionResult: unknown;
		const client = createClient({
			enroll: async (_input, observer, signal) => {
				operationSignal = signal;
				interactionResult = await observer.interact({
					generation: 1,
					handle: 'prompt-owned',
					kind: 'confirm',
					prompt: 'Trust host?',
				});
				return { kind: 'cancelled' };
			},
		});
		const controller = new DesktopSshProjectEnrollmentController({
			client,
			onEnrollmentCommitted: async () => true,
		});
		openValidEnrollment(controller);
		controller.submit();
		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({ phase: 'prompt' }),
		);

		controller.close();
		await vi.waitFor(() => expect(interactionResult).toEqual({ kind: 'cancelled' }));
		expect(operationSignal?.aborted).toBe(true);
		expect(controller.getDialogSnapshot()).toEqual({ kind: 'closed' });
		controller.dispose();
	});

	it('publishes stable Project status and retries a failed explicit connection', async () => {
		const statusHarness = statusSource([
			{ blockedReason: null, projectId: 'project-remote', state: 'disconnected' },
		]);
		const client = createClient({
			connect: vi
				.fn<DesktopSshProjectEnrollmentControllerClient['connect']>()
				.mockResolvedValueOnce({ kind: 'failed', reason: 'remote-runtime-unavailable' })
				.mockResolvedValueOnce({ kind: 'connected', projectId: 'project-remote' }),
			getStatuses: statusHarness.getStatuses,
			subscribeStatuses: statusHarness.subscribeStatuses,
		});
		const controller = new DesktopSshProjectEnrollmentController({
			client,
			onEnrollmentCommitted: async () => true,
		});
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);
		expect(controller.getSnapshot()).toEqual([
			{ projectId: 'project-remote', state: 'disconnected' },
		]);
		statusHarness.publish([
			{ blockedReason: null, projectId: 'project-remote', state: 'retrying' },
		]);
		expect(controller.getSnapshot()).toEqual([{ projectId: 'project-remote', state: 'retrying' }]);
		expect(listener).toHaveBeenCalledOnce();

		controller.connect('project-remote');
		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({
				failure: 'connection',
				kind: 'connection',
				phase: 'failed',
			}),
		);
		controller.retry();
		await vi.waitFor(() => expect(controller.getDialogSnapshot()).toEqual({ kind: 'closed' }));
		expect(client.connect).toHaveBeenCalledTimes(2);
		controller.dispose();
		expect(client.dispose).toHaveBeenCalledOnce();
	});

	it('keeps a committed enrollment recoverable when canonical publication is unavailable', async () => {
		const client = createClient({ enroll: async () => enrollmentCreated() });
		const committed = vi
			.fn<DesktopSshProjectEnrollmentControllerOptions['onEnrollmentCommitted']>()
			.mockResolvedValueOnce(false)
			.mockResolvedValueOnce(true);
		const controller = new DesktopSshProjectEnrollmentController({
			client,
			onEnrollmentCommitted: committed,
		});
		openValidEnrollment(controller);
		controller.submit();

		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({
				failure: 'workbench',
				kind: 'enrollment',
				phase: 'failed',
			}),
		);
		controller.retry();
		await vi.waitFor(() => expect(controller.getDialogSnapshot()).toEqual({ kind: 'closed' }));
		expect(client.enroll).toHaveBeenCalledTimes(2);
		expect(committed).toHaveBeenCalledTimes(2);
		controller.dispose();
	});
});

function createClient(
	overrides: Partial<DesktopSshProjectEnrollmentControllerClient>,
): DesktopSshProjectEnrollmentControllerClient {
	return {
		connect: vi.fn<DesktopSshProjectEnrollmentControllerClient['connect']>(
			overrides.connect ?? (async (projectId) => ({ kind: 'connected', projectId })),
		),
		disconnect: vi.fn<DesktopSshProjectEnrollmentControllerClient['disconnect']>(
			overrides.disconnect ?? (async (projectId) => ({ kind: 'disconnected', projectId })),
		),
		dispose: vi.fn<DesktopSshProjectEnrollmentControllerClient['dispose']>(
			overrides.dispose ?? (() => undefined),
		),
		enroll: vi.fn<DesktopSshProjectEnrollmentControllerClient['enroll']>(
			overrides.enroll ?? (async () => ({ kind: 'cancelled' })),
		),
		getStatuses: vi.fn<DesktopSshProjectEnrollmentControllerClient['getStatuses']>(
			overrides.getStatuses ?? (() => []),
		),
		subscribeStatuses: vi.fn<DesktopSshProjectEnrollmentControllerClient['subscribeStatuses']>(
			overrides.subscribeStatuses ?? (() => () => undefined),
		),
	};
}

function enrollmentCreated(): DesktopSshProjectEnrollmentResult {
	return {
		canonicalPath: '/srv/orchbee',
		kind: 'created',
		projectId: 'project-remote',
		workspaceId: 'workspace-primary',
	};
}

function openValidEnrollment(controller: DesktopSshProjectEnrollmentController): void {
	controller.openEnrollment();
	controller.updateForm({
		destination: 'build-host',
		name: 'Build machine',
		projectPath: '/srv/orchbee',
	});
}

function statusSource(initial: readonly DesktopSshProjectConnectionStatus[]) {
	let statuses = initial;
	const listeners = new Set<() => void>();
	return {
		getStatuses: () => statuses,
		publish(next: readonly DesktopSshProjectConnectionStatus[]): void {
			statuses = next;
			for (const listener of listeners) listener();
		},
		subscribeStatuses(listener: () => void): () => void {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}
