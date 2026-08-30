import type {
	DesktopSshProjectConnectionStatus,
	DesktopSshProjectDirectorySnapshot,
	DesktopSshProjectEnrollmentResult,
} from '@orchbee/desktop-client-protocol';
import {
	DesktopSshEnvironmentCatalogSchema,
	DesktopSshEnvironmentIdSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopSshProjectEnrollmentController,
	type DesktopSshProjectEnrollmentControllerClient,
	type DesktopSshProjectEnrollmentControllerOptions,
} from '../DesktopSshProjectEnrollmentController';

const confirmFolderAttachment: DesktopSshProjectEnrollmentControllerOptions['onFolderAttachmentCommitted'] =
	async () => true;

describe('DesktopSshProjectEnrollmentController', () => {
	it('refreshes configured hosts on entry without replacing the current address draft', () => {
		const catalogSource = environmentCatalogSource();
		const client = createClient({
			getEnvironmentCatalog: catalogSource.getEnvironmentCatalog,
			subscribeEnvironmentCatalog: catalogSource.subscribeEnvironmentCatalog,
		});
		const controller = new DesktopSshProjectEnrollmentController({
			client,
			onEnrollmentCommitted: async () => true,
			onFolderAttachmentCommitted: confirmFolderAttachment,
		});
		const catalogListener = vi.fn<() => void>();
		controller.subscribeDestinationCatalog(catalogListener);

		controller.openEnrollment();
		expect(client.refreshEnvironmentCatalog).toHaveBeenCalledExactlyOnceWith(true);
		expect(controller.getDestinationCatalogSnapshot().suggestions).toEqual([
			{ destination: 'gpu-lab' },
		]);
		controller.updateForm({ destination: 'builder@custom-host' });
		catalogSource.publish(
			DesktopSshEnvironmentCatalogSchema.parse({
				discoveryIssue: null,
				managed: [],
				registryIssue: null,
				suggestions: [{ destination: 'build-host' }],
			}),
		);

		expect(catalogListener).toHaveBeenCalledOnce();
		expect(controller.getDestinationCatalogSnapshot().suggestions).toEqual([
			{ destination: 'build-host' },
		]);
		expect(controller.getDialogSnapshot()).toMatchObject({
			form: { destination: 'builder@custom-host' },
		});
		controller.dispose();
	});

	it('validates enrollment locally and keeps authentication responses outside dialog state', async () => {
		const responses: unknown[] = [];
		const directorySelections: unknown[] = [];
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
				directorySelections.push(await observer.selectDirectory(directorySnapshot('/srv/orchbee')));
				return enrollmentCreated();
			},
		});
		const committed = vi.fn<DesktopSshProjectEnrollmentControllerOptions['onEnrollmentCommitted']>(
			async () => true,
		);
		const controller = new DesktopSshProjectEnrollmentController({
			client,
			onEnrollmentCommitted: committed,
			onFolderAttachmentCommitted: confirmFolderAttachment,
		});

		controller.openEnrollment();
		expect(controller.getDialogSnapshot()).toMatchObject({ canSubmit: false, phase: 'editing' });
		controller.updateForm({ destination: ' ssh builder@build-host ' });
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
		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({
				directory: { canonicalPath: '/srv/orchbee' },
				phase: 'directory',
			}),
		);
		controller.selectDirectory();

		await vi.waitFor(() => expect(controller.getDialogSnapshot()).toEqual({ kind: 'closed' }));
		expect(client.enroll).toHaveBeenCalledWith(
			{ destination: 'ssh builder@build-host' },
			expect.any(Object),
			expect.any(AbortSignal),
		);
		expect(responses).toEqual([{ kind: 'submitted', response: 'one-use secret' }]);
		expect(directorySelections).toEqual([{ kind: 'selected' }]);
		expect(committed).toHaveBeenCalledWith(enrollmentCreated());
		controller.dispose();
	});

	it('keeps the last valid directory visible when navigation fails and resumes selection', async () => {
		const selections: unknown[] = [];
		const client = createClient({
			enroll: async (_input, observer) => {
				selections.push(await observer.selectDirectory(directorySnapshot('/home/builder')));
				selections.push(
					await observer.selectDirectory({
						...directorySnapshot('/home/builder'),
						navigationFailure: 'not-found',
					}),
				);
				selections.push(await observer.selectDirectory(directorySnapshot('/srv/orchbee')));
				return enrollmentCreated();
			},
		});
		const controller = new DesktopSshProjectEnrollmentController({
			client,
			onEnrollmentCommitted: async () => true,
			onFolderAttachmentCommitted: confirmFolderAttachment,
		});
		openValidEnrollment(controller);
		controller.submit();
		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({
				directory: { canonicalPath: '/home/builder' },
				phase: 'directory',
			}),
		);

		controller.updateDirectoryPath('/missing');
		controller.navigateDirectory('/missing');
		expect(controller.getDialogSnapshot()).toMatchObject({
			directory: { canonicalPath: '/home/builder', navigationFailure: null },
			pathInput: '/missing',
			phase: 'working',
		});
		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({
				directory: {
					canonicalPath: '/home/builder',
					navigationFailure: 'not-found',
				},
				pathInput: '/missing',
				phase: 'directory',
			}),
		);

		controller.updateDirectoryPath('/srv/orchbee');
		controller.navigateDirectory('/srv/orchbee');
		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({
				directory: { canonicalPath: '/srv/orchbee', navigationFailure: null },
				pathInput: '/srv/orchbee',
				phase: 'directory',
			}),
		);
		controller.selectDirectory();

		await vi.waitFor(() => expect(controller.getDialogSnapshot()).toEqual({ kind: 'closed' }));
		expect(selections).toEqual([
			{ kind: 'navigate', path: '/missing' },
			{ kind: 'navigate', path: '/srv/orchbee' },
			{ kind: 'selected' },
		]);
		controller.dispose();
	});

	it('attaches to the exact Project and closes only after canonical publication', async () => {
		const publication = createDeferred<boolean>();
		const result = {
			canonicalPath: '/srv/shared',
			directoryId: 'environment-directory:44444444444444444444444444444444',
			kind: 'attached' as const,
			projectId: 'project-local',
		};
		const client = createClient({
			attachFolder: async (_projectId, _input, observer) => {
				await observer.selectDirectory(directorySnapshot('/srv/shared'));
				return result;
			},
		});
		const committed = vi.fn<
			DesktopSshProjectEnrollmentControllerOptions['onFolderAttachmentCommitted']
		>(() => publication.promise);
		const controller = new DesktopSshProjectEnrollmentController({
			client,
			onEnrollmentCommitted: async () => true,
			onFolderAttachmentCommitted: committed,
		});

		controller.openFolderAttachment('project-local');
		expect(controller.getDialogSnapshot()).toMatchObject({
			kind: 'attachment',
			phase: 'editing',
			projectId: 'project-local',
		});
		controller.updateForm({ destination: 'ssh builder@host' });
		controller.submit();
		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({
				directory: { canonicalPath: '/srv/shared' },
				kind: 'attachment',
				phase: 'directory',
			}),
		);
		controller.selectDirectory();
		await vi.waitFor(() => expect(committed).toHaveBeenCalledWith(result));
		expect(controller.getDialogSnapshot()).toMatchObject({
			kind: 'attachment',
			phase: 'working',
		});

		publication.resolve(true);
		await vi.waitFor(() => expect(controller.getDialogSnapshot()).toEqual({ kind: 'closed' }));
		expect(client.attachFolder).toHaveBeenCalledWith(
			'project-local',
			{ destination: 'ssh builder@host' },
			expect.any(Object),
			expect.any(AbortSignal),
		);
		expect(client.enroll).not.toHaveBeenCalled();
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
			onFolderAttachmentCommitted: confirmFolderAttachment,
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

	it('cancels an enrollment substep back to the retained destination and fences late settlement', async () => {
		const completion = createDeferred<DesktopSshProjectEnrollmentResult>();
		let interactionResult: unknown;
		let operationCompleted = false;
		let operationSignal: AbortSignal | undefined;
		const client = createClient({
			enroll: async (_input, observer, signal) => {
				operationSignal = signal;
				interactionResult = await observer.interact({
					generation: 1,
					handle: 'prompt-cancelled-to-editing',
					kind: 'none',
					prompt: 'Password:',
				});
				const result = await completion.promise;
				operationCompleted = true;
				return result;
			},
		});
		const controller = new DesktopSshProjectEnrollmentController({
			client,
			onEnrollmentCommitted: async () => true,
			onFolderAttachmentCommitted: confirmFolderAttachment,
		});
		controller.openEnrollment();
		controller.updateForm({ destination: 'ssh builder@build-host -A' });
		controller.submit();
		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({ phase: 'prompt' }),
		);

		controller.cancel();
		await vi.waitFor(() => expect(interactionResult).toEqual({ kind: 'cancelled' }));
		expect(operationSignal?.aborted).toBe(true);
		expect(controller.getDialogSnapshot()).toMatchObject({
			form: { destination: 'ssh builder@build-host -A' },
			kind: 'enrollment',
			phase: 'editing',
		});

		completion.resolve({ kind: 'cancelled' });
		await vi.waitFor(() => expect(operationCompleted).toBe(true));
		expect(controller.getDialogSnapshot()).toMatchObject({
			form: { destination: 'ssh builder@build-host -A' },
			kind: 'enrollment',
			phase: 'editing',
		});
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
			onFolderAttachmentCommitted: confirmFolderAttachment,
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
			onFolderAttachmentCommitted: confirmFolderAttachment,
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

	it('manages reusable Environments without connecting until an explicit action', async () => {
		const environmentId = DesktopSshEnvironmentIdSchema.parse(`environment:${'a'.repeat(32)}`);
		const catalogSource = environmentCatalogSource();
		const promptResponses: unknown[] = [];
		const client = createClient({
			addEnvironment: async () => ({ environmentId, kind: 'added' }),
			connectEnvironment: async (_candidate, observer) => {
				promptResponses.push(
					await observer.interact({
						generation: 2,
						handle: 'environment-prompt',
						kind: 'none',
						prompt: 'Password:',
					}),
				);
				return { environmentId, kind: 'connected' };
			},
			disconnectEnvironment: async () => ({ environmentId, kind: 'disconnected' }),
			getEnvironmentCatalog: catalogSource.getEnvironmentCatalog,
			subscribeEnvironmentCatalog: catalogSource.subscribeEnvironmentCatalog,
		});
		const controller = new DesktopSshProjectEnrollmentController({
			client,
			onEnrollmentCommitted: async () => true,
			onFolderAttachmentCommitted: confirmFolderAttachment,
		});
		const managementListener = vi.fn<() => void>();
		controller.subscribeManagement(managementListener);

		await expect(controller.addConnection(' gpu-lab ')).resolves.toBe(true);
		expect(client.addEnvironment).toHaveBeenCalledWith(
			{ destination: 'gpu-lab' },
			expect.any(AbortSignal),
		);
		expect(client.connectEnvironment).not.toHaveBeenCalled();
		catalogSource.publish(
			DesktopSshEnvironmentCatalogSchema.parse({
				discoveryIssue: null,
				managed: [
					{
						blockedReason: null,
						destination: 'gpu-lab',
						environmentId,
						name: 'gpu-lab',
						projectCount: 0,
						state: 'disconnected',
					},
				],
				registryIssue: null,
				suggestions: [],
			}),
		);
		expect(controller.getManagementSnapshot().catalog.managed).toHaveLength(1);
		expect(managementListener).toHaveBeenCalled();

		controller.connectEnvironment(environmentId, 'gpu-lab');
		await vi.waitFor(() =>
			expect(controller.getDialogSnapshot()).toMatchObject({
				environmentId,
				kind: 'environment-connection',
				phase: 'prompt',
			}),
		);
		expect(JSON.stringify(controller.getManagementSnapshot())).not.toContain('one-use secret');
		controller.resolvePrompt({ kind: 'submitted', response: 'one-use secret' });
		await vi.waitFor(() => expect(controller.getDialogSnapshot()).toEqual({ kind: 'closed' }));
		expect(promptResponses).toEqual([{ kind: 'submitted', response: 'one-use secret' }]);

		controller.disconnectEnvironment(environmentId);
		await vi.waitFor(() => expect(client.disconnectEnvironment).toHaveBeenCalledOnce());
		await vi.waitFor(() => expect(controller.getManagementSnapshot().pending).toBeNull());
		controller.dispose();
	});
});

function createClient(
	overrides: Partial<DesktopSshProjectEnrollmentControllerClient>,
): DesktopSshProjectEnrollmentControllerClient {
	return {
		addEnvironment: vi.fn<DesktopSshProjectEnrollmentControllerClient['addEnvironment']>(
			overrides.addEnvironment ?? (async () => ({ kind: 'cancelled' })),
		),
		attachFolder: vi.fn<DesktopSshProjectEnrollmentControllerClient['attachFolder']>(
			overrides.attachFolder ?? (async () => ({ kind: 'cancelled' })),
		),
		connect: vi.fn<DesktopSshProjectEnrollmentControllerClient['connect']>(
			overrides.connect ?? (async (projectId) => ({ kind: 'connected', projectId })),
		),
		connectEnvironment: vi.fn<DesktopSshProjectEnrollmentControllerClient['connectEnvironment']>(
			overrides.connectEnvironment ?? (async () => ({ kind: 'cancelled' })),
		),
		disconnect: vi.fn<DesktopSshProjectEnrollmentControllerClient['disconnect']>(
			overrides.disconnect ?? (async (projectId) => ({ kind: 'disconnected', projectId })),
		),
		disconnectEnvironment: vi.fn<
			DesktopSshProjectEnrollmentControllerClient['disconnectEnvironment']
		>(overrides.disconnectEnvironment ?? (async () => ({ kind: 'cancelled' }))),
		dispose: vi.fn<DesktopSshProjectEnrollmentControllerClient['dispose']>(
			overrides.dispose ?? (() => undefined),
		),
		enroll: vi.fn<DesktopSshProjectEnrollmentControllerClient['enroll']>(
			overrides.enroll ?? (async () => ({ kind: 'cancelled' })),
		),
		getEnvironmentCatalog: vi.fn<
			DesktopSshProjectEnrollmentControllerClient['getEnvironmentCatalog']
		>(
			overrides.getEnvironmentCatalog ??
				(() => ({
					discoveryIssue: null,
					managed: [],
					registryIssue: null,
					suggestions: [],
				})),
		),
		getStatuses: vi.fn<DesktopSshProjectEnrollmentControllerClient['getStatuses']>(
			overrides.getStatuses ?? (() => []),
		),
		refreshEnvironmentCatalog: vi.fn<
			DesktopSshProjectEnrollmentControllerClient['refreshEnvironmentCatalog']
		>(overrides.refreshEnvironmentCatalog ?? (() => undefined)),
		subscribeEnvironmentCatalog: vi.fn<
			DesktopSshProjectEnrollmentControllerClient['subscribeEnvironmentCatalog']
		>(overrides.subscribeEnvironmentCatalog ?? (() => () => undefined)),
		subscribeStatuses: vi.fn<DesktopSshProjectEnrollmentControllerClient['subscribeStatuses']>(
			overrides.subscribeStatuses ?? (() => () => undefined),
		),
	};
}

function environmentCatalogSource() {
	let catalog = DesktopSshEnvironmentCatalogSchema.parse({
		discoveryIssue: null,
		managed: [],
		registryIssue: null,
		suggestions: [{ destination: 'gpu-lab' }],
	});
	const listeners = new Set<() => void>();
	return {
		getEnvironmentCatalog: () => catalog,
		publish: (next: typeof catalog): void => {
			catalog = next;
			for (const listener of listeners) listener();
		},
		subscribeEnvironmentCatalog: (listener: () => void): (() => void) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
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
	controller.updateForm({ destination: 'build-host' });
}

function directorySnapshot(canonicalPath: string): DesktopSshProjectDirectorySnapshot {
	return {
		canonicalPath,
		directories: [{ name: 'project' }],
		navigationFailure: null,
		parentPath: canonicalPath === '/' ? null : '/',
		truncated: false,
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
