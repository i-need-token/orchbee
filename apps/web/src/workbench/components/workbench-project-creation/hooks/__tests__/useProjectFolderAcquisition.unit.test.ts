// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionResult,
	type ActionSignature,
} from '@orchbee/actions';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
	WorkbenchProjectFolderAcquisition,
	WorkbenchProjectFolderAcquisitionResult,
	WorkbenchNewProjectFolderCapability,
	WorkbenchProjectCreationReceipt,
} from '../../../../types/workbench-project-folder-acquisition';
import { useProjectFolderAcquisition } from '../useProjectFolderAcquisition';
import { useNewProjectFolder } from '../useNewProjectFolder';
import { useWorkbenchProjectCreation } from '../useWorkbenchProjectCreation';

afterEach(cleanup);

it.each([true, false])(
	'creates a Workspace in its captured Project through folder selection or path entry: %s',
	async (native) => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workspace-directory');
		const createWorkspaceAtDirectory = vi.fn<(projectId: string, path: string) => void>();
		const project = testAction<WorkbenchProjectCreationReceipt>({
			kind: 'blocked',
			code: 'unexpected-project',
		});
		const createProject = vi.fn<typeof project.execute>(() => project.execute());
		const rendered = renderHook(() =>
			useWorkbenchProjectCreation({
				acquisition: native
					? { selectExistingFolder: () => Promise.resolve({ kind: 'selected', path: '/chosen' }) }
					: undefined,
				activateCreatedProject: () => testAction<void>({ kind: 'completed' }).execute(),
				createProject,
				createWorkspaceAtDirectory,
				isCreatedProjectActive: () => false,
				runtime,
				scope,
			}),
		);
		await act(async () =>
			rendered.result.current.requestWorkspaceDirectory(
				'addressed-project',
				'New Workspace in directory',
			),
		);
		expect(rendered.result.current.pathForm?.title).toBe(
			native ? undefined : 'New Workspace in directory',
		);
		if (!native) {
			act(() => rendered.result.current.pathForm?.submit('/chosen'));
		}
		expect(createWorkspaceAtDirectory).toHaveBeenCalledExactlyOnceWith(
			'addressed-project',
			'/chosen',
		);
		expect(createProject).not.toHaveBeenCalled();
		expect(rendered.result.current.pathForm).toBeNull();
	},
);

describe('New Folder Action ownership', () => {
	it.each([true, false])(
		'waits for actual activation acceptance and rechecks canonical receipt: %s',
		async (stillPublished) => {
			const capability = newFolderCapability();
			capability.confirmProject.mockResolvedValueOnce(true).mockResolvedValueOnce(stillPublished);
			const receipt = {
				directoryId: 'folder',
				path: '/workspace/new',
				presentationId: 'unique-first',
				projectId: 'project-1',
				terminalContextId: 'context-1',
				workspaceId: 'workspace-1',
			};
			const creation = testAction<WorkbenchProjectCreationReceipt>({
				kind: 'completed',
				value: receipt,
			});
			let accept: ((result: ActionResult<void>) => void) | undefined;
			const activation = testAction<void>(
				new Promise<ActionResult<void>>((resolve) => {
					accept = resolve;
				}),
			);
			let active = false;
			const rendered = renderHook(() =>
				useNewProjectFolder({
					capability,
					createProject: () => creation.execute(),
					activateCreatedProject: () => activation.execute(),
					isCreatedProjectActive: () => active,
					selectExisting: vi.fn<() => void>(),
				}),
			);
			await act(async () => rendered.result.current.open());
			act(() => rendered.result.current.setName('new'));
			await act(async () => rendered.result.current.submit());
			await waitFor(() => expect(activation.handler).toHaveBeenCalledOnce());
			expect(rendered.result.current.state.kind).toBe('opening');
			expect(capability.acknowledge).not.toHaveBeenCalled();
			await act(async () => {
				active = true;
				accept?.({ kind: 'completed' });
			});
			expect(capability.confirmProject).toHaveBeenCalledTimes(2);
			expect(rendered.result.current.state.kind).toBe(stillPublished ? 'idle' : 'recovery');
			expect(capability.acknowledge.mock.calls).toEqual(stillPublished ? [['create:1']] : []);
			rendered.unmount();
			creation.dispose();
			activation.dispose();
		},
	);

	it('retains the created path when the actual Project Action rejects instead of treating invocation as success', async () => {
		const capability = newFolderCapability();
		const creation = testAction<WorkbenchProjectCreationReceipt>({
			code: 'unavailable',
			kind: 'blocked',
		});
		const activation = testAction<void>({ kind: 'completed' });
		const activateCreatedProject = vi.fn<typeof activation.execute>(() => activation.execute());
		const rendered = renderHook(() =>
			useNewProjectFolder({
				capability,
				createProject: () => creation.execute(),
				activateCreatedProject,
				isCreatedProjectActive: () => true,
				selectExisting: vi.fn<() => void>(),
			}),
		);
		await act(async () => rendered.result.current.open());
		act(() => rendered.result.current.setName('new'));
		await act(async () => rendered.result.current.submit());
		await waitFor(() =>
			expect(rendered.result.current.state).toMatchObject({
				kind: 'recovery',
				path: '/workspace/new',
			}),
		);
		expect(capability.confirmProject).not.toHaveBeenCalled();
		expect(activateCreatedProject).not.toHaveBeenCalled();
		expect(capability.acknowledge).not.toHaveBeenCalled();
		act(() => rendered.result.current.close());
		expect(capability.acknowledge).toHaveBeenCalledWith('create:1');
		rendered.unmount();
		creation.dispose();
		activation.dispose();
	});

	it('cancels a queued Project Action when the creating workflow unmounts', async () => {
		const capability = newFolderCapability();
		let finish: ((value: ActionResult<WorkbenchProjectCreationReceipt>) => void) | undefined;
		const pending = new Promise<ActionResult<WorkbenchProjectCreationReceipt>>((resolve) => {
			finish = resolve;
		});
		const creation = testAction(pending);
		const preceding = creation.execute();
		const activation = testAction<void>({ kind: 'completed' });
		const activateCreatedProject = vi.fn<typeof activation.execute>(() => activation.execute());
		let queued: ReturnType<typeof creation.execute> | undefined;
		const rendered = renderHook(() =>
			useNewProjectFolder({
				capability,
				createProject: () => {
					queued = creation.execute();
					return queued;
				},
				activateCreatedProject,
				isCreatedProjectActive: () => true,
				selectExisting: vi.fn<() => void>(),
			}),
		);
		await act(async () => rendered.result.current.open());
		act(() => rendered.result.current.setName('new'));
		await act(async () => rendered.result.current.submit());
		await waitFor(() => expect(rendered.result.current.state.kind).toBe('opening'));
		rendered.unmount();
		finish?.({ code: 'preceding-finished', kind: 'blocked' });
		await preceding.completion;
		await expect(queued?.completion).resolves.toMatchObject({ kind: 'cancelled' });
		expect(creation.handler).toHaveBeenCalledTimes(1);
		expect(capability.releaseParent).toHaveBeenCalledWith({
			path: '/workspace',
			selectionId: 'parent:1',
		});
		expect(activateCreatedProject).not.toHaveBeenCalled();
		creation.dispose();
		activation.dispose();
	});

	it.each(['retire', 'replace'] as const)(
		'does not activate an old publication after capability %s',
		async (change) => {
			const lifetime = new AbortController();
			const capability = newFolderCapability(lifetime.signal);
			let publish: ((accepted: boolean) => void) | undefined;
			capability.confirmProject.mockImplementation(
				() =>
					new Promise<boolean>((resolve) => {
						publish = resolve;
					}),
			);
			const receipt = {
				directoryId: 'folder',
				path: '/workspace/new',
				presentationId: 'unique-first',
				projectId: 'project-1',
				terminalContextId: 'context-1',
				workspaceId: 'workspace-1',
			};
			const creation = testAction<WorkbenchProjectCreationReceipt>({
				kind: 'completed',
				value: receipt,
			});
			const activation = testAction<void>({ kind: 'completed' });
			const activateCreatedProject = vi.fn<typeof activation.execute>(() => activation.execute());
			const rendered = renderHook(
				({ selectedCapability }) =>
					useNewProjectFolder({
						capability: selectedCapability,
						createProject: () => creation.execute(),
						activateCreatedProject,
						isCreatedProjectActive: () => true,
						selectExisting: vi.fn<() => void>(),
					}),
				{ initialProps: { selectedCapability: capability } },
			);
			await act(async () => rendered.result.current.open());
			act(() => rendered.result.current.setName('new'));
			await act(async () => rendered.result.current.submit());
			await waitFor(() => expect(capability.confirmProject).toHaveBeenCalledWith(receipt));
			if (change === 'retire') {
				act(() => lifetime.abort());
			} else {
				rendered.rerender({ selectedCapability: newFolderCapability() });
				await act(async () => rendered.result.current.open());
			}
			await act(async () => publish?.(true));
			expect(rendered.result.current.state).toMatchObject(
				change === 'retire' ? { kind: 'recovery', path: '/workspace/new' } : { kind: 'draft' },
			);
			expect(activateCreatedProject).not.toHaveBeenCalled();
			expect(capability.acknowledge).not.toHaveBeenCalled();
			rendered.unmount();
			creation.dispose();
			activation.dispose();
		},
	);
});

function newFolderCapability(signal = new AbortController().signal) {
	return {
		acknowledge: vi.fn<WorkbenchNewProjectFolderCapability['acknowledge']>(),
		confirmProject: vi.fn<WorkbenchNewProjectFolderCapability['confirmProject']>(async () => true),
		create: vi.fn<WorkbenchNewProjectFolderCapability['create']>(async () => ({
			kind: 'created',
			path: '/workspace/new',
			requestId: 'create:1',
		})),
		releaseParent: vi.fn<WorkbenchNewProjectFolderCapability['releaseParent']>(),
		selectParent: vi.fn<WorkbenchNewProjectFolderCapability['selectParent']>(async () => ({
			kind: 'parent-selected',
			parent: { path: '/workspace', selectionId: 'parent:1' },
		})),
		signal,
	};
}

function testAction<Output>(result: ActionResult<Output> | Promise<ActionResult<Output>>) {
	const runtime = new ActionRuntime();
	const typed = createTypedActionRuntime<{ test: ActionSignature<void, Output> }>(runtime);
	const scope = runtime.createScope('test');
	const handler = vi.fn<() => ActionResult<Output> | Promise<ActionResult<Output>>>(() => result);
	const registration = typed.register(
		scope,
		defineAction({
			category: { id: 'test', title: { id: 'test.category' } },
			execution: { concurrency: 'serialize', concurrencyKey: () => 'same', mode: 'foreground' },
			getAvailability: () => ({ kind: 'enabled' }),
			handler: { execute: handler },
			id: 'test',
			input: noActionInput,
			title: { id: 'test.title' },
		}),
	);
	return {
		dispose: () => {
			registration.dispose();
			runtime.dispose();
		},
		execute: () => typed.execute('test', undefined, { scope, source: 'button' }),
		handler,
	};
}

describe('useProjectFolderAcquisition', () => {
	it('commits a selected Folder once and suppresses overlapping requests', async () => {
		let settle: ((result: WorkbenchProjectFolderAcquisitionResult) => void) | undefined;
		const acquisition: WorkbenchProjectFolderAcquisition = {
			selectExistingFolder: vi.fn<() => Promise<WorkbenchProjectFolderAcquisitionResult>>(
				() =>
					new Promise<WorkbenchProjectFolderAcquisitionResult>((resolve) => {
						settle = resolve;
					}),
			),
		};
		const createProject = vi.fn<(path: string) => void>();
		const rendered = renderHook(() => useProjectFolderAcquisition({ acquisition, createProject }));

		act(() => {
			rendered.result.current.requestCreateProject();
			rendered.result.current.requestCreateProject();
		});
		expect(acquisition.selectExistingFolder).toHaveBeenCalledOnce();
		expect(rendered.result.current.selecting).toBe(true);
		await act(async () => {
			settle?.({ kind: 'selected', path: '/workspace/orchbee' });
			await Promise.resolve();
		});

		expect(createProject).toHaveBeenCalledWith('/workspace/orchbee');
		expect(rendered.result.current.selecting).toBe(false);
	});

	it('ignores a selection that settles after unmount', async () => {
		let settle: ((result: WorkbenchProjectFolderAcquisitionResult) => void) | undefined;
		const acquisition: WorkbenchProjectFolderAcquisition = {
			selectExistingFolder: () =>
				new Promise<WorkbenchProjectFolderAcquisitionResult>((resolve) => {
					settle = resolve;
				}),
		};
		const createProject = vi.fn<(path: string) => void>();
		const rendered = renderHook(() => useProjectFolderAcquisition({ acquisition, createProject }));

		act(() => rendered.result.current.requestCreateProject());
		rendered.unmount();
		settle?.({ kind: 'selected', path: '/workspace/late' });
		await Promise.resolve();
		expect(createProject).not.toHaveBeenCalled();
	});
});
