import type { ActionExecutionContext } from '@orchbee/actions';
import { workbenchSharedDocumentRevision } from '../../document';
import { describe, expect, it, vi } from 'vitest';

import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '../../../terminal-presentation';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchRemoteTerminalCreationCapability,
	WorkbenchRemoteTerminalCreationResult,
} from '../../types/workbench-remote-terminal-creation';
import {
	executeWorkbenchTerminalCreationAction,
	getWorkbenchTabCreationAvailability,
	getWorkbenchTerminalCreationAvailability,
} from '../workbench-terminal-creation-action';

const ENVIRONMENT_ID = 'environment:00000000000000000000000000000001';

function createRemoteRuntime(scope: string): WorkbenchProjectWorkspaceRuntime {
	let operationSequence = 0;
	let presentationSequence = 0;
	let removalSequence = 0;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Terminal 1',
		terminalPresentation: {
			createOperationId: () =>
				terminalPresentationOperationId(`${scope}:operation:${++operationSequence}`),
			createPresentationId: () =>
				terminalPresentationId(`${scope}:presentation:${++presentationSequence}`),
			createRemovalId: () => terminalPresentationRemovalId(`${scope}:removal:${++removalSequence}`),
			dispatchCommands: () => undefined,
			initialPresentationId: terminalPresentationId(`${scope}:initial`),
			kind: 'managed',
		},
	});
	runtime.transition({
		directory: {
			available: false,
			git: null,
			id: 'remote-directory-1',
			kind: 'folder',
			name: 'orchbee',
			path: '/srv/orchbee',
			placement: { environmentId: ENVIRONMENT_ID, kind: 'environment' },
			source: 'selected',
		},
		projectName: 'orchbee',
		type: 'environment-project-registered',
	});
	runtime.transition({
		directoryId: 'remote-directory-1',
		projectId: 'project-1',
		type: 'environment-workspace-registered',
		workspaceName: 'orchbee',
	});
	runtime.transition({
		projectId: 'project-1',
		type: 'workspace-activated',
		workspaceId: 'workspace-1',
	});
	return runtime;
}

function createContext(abortController = new AbortController()): {
	readonly context: ActionExecutionContext;
	readonly markCommitted: ReturnType<typeof vi.fn<() => void>>;
} {
	const markCommitted = vi.fn<() => void>();
	return {
		context: {
			executionId: 'execution-1',
			markCommitted,
			reportProgress: () => undefined,
			scope: { id: 'workbench-test', parent: null },
			signal: abortController.signal,
			source: 'button',
		},
		markCommitted,
	};
}

function createMainResult(runtime: WorkbenchProjectWorkspaceRuntime) {
	const creation = runtime.createEnvironmentTerminalAtLocation({
		environmentId: ENVIRONMENT_ID,
		fallbackTitle: 'Remote Terminal',
		placement: { groupId: null },
		projectId: 'project-1',
		workspaceId: 'workspace-1',
	});
	if (creation.kind !== 'created') throw new Error('Expected Main to create the remote Terminal.');
	return {
		...creation,
		kind: 'created' as const,
		projectId: 'project-1',
		revision: workbenchSharedDocumentRevision('1'),
		workspaceId: 'workspace-1',
	};
}

function capabilityReturning(
	result: WorkbenchRemoteTerminalCreationResult,
): WorkbenchRemoteTerminalCreationCapability {
	return { create: vi.fn<WorkbenchRemoteTerminalCreationCapability['create']>(async () => result) };
}

describe('executeWorkbenchTerminalCreationAction', () => {
	it('enables each default top-level remote Terminal surface when the capability exists', () => {
		const runtime = createRemoteRuntime('availability');
		const capability = capabilityReturning({ kind: 'cancelled' });

		expect(getWorkbenchTerminalCreationAvailability(runtime, undefined, { groupId: null })).toEqual(
			{
				kind: 'disabled',
				reason: expect.any(Object),
			},
		);
		expect(
			getWorkbenchTerminalCreationAvailability(runtime, capability, { groupId: null }),
		).toEqual({ kind: 'enabled' });
		expect(
			getWorkbenchTerminalCreationAvailability(runtime, capability, {
				groupId: 'group-1',
			}),
		).toMatchObject({ kind: 'disabled' });
		expect(
			getWorkbenchTabCreationAvailability(runtime, capability, { kind: 'new-group' }),
		).toMatchObject({ kind: 'disabled' });
		expect(
			getWorkbenchTabCreationAvailability(runtime, capability, {
				kind: 'duplicate',
				tabIds: ['tab-1'],
			}),
		).toMatchObject({ kind: 'disabled' });
		createMainResult(runtime);
		expect(
			getWorkbenchTerminalCreationAvailability(runtime, capability, { groupId: null }),
		).toEqual({ kind: 'enabled' });
	});

	it('waits for canonical publication without an optimistic Renderer mutation', async () => {
		const rendererRuntime = createRemoteRuntime('renderer');
		const mainRuntime = createRemoteRuntime('main');
		const mainResult = createMainResult(mainRuntime);
		const capability = capabilityReturning(mainResult);
		const { context, markCommitted } = createContext();
		const onRemoteTerminalCreated =
			vi.fn<(creation: { readonly tabId: string; readonly terminalContextId: string }) => void>();
		let settled = false;
		const completion = Promise.resolve(
			executeWorkbenchTerminalCreationAction(
				context,
				{ groupId: null },
				{
					fallbackTitle: 'Remote Terminal',
					onRemoteTerminalCreated,
					remoteTerminalCreation: capability,
					runtime: rendererRuntime,
				},
			),
		).then((result) => {
			settled = true;
			return result;
		});

		await Promise.resolve();
		expect(settled).toBe(false);
		expect(rendererRuntime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toEqual([]);
		expect(markCommitted).toHaveBeenCalledOnce();

		rendererRuntime.applySharedDocument(
			mainRuntime.getSharedDocument(workbenchSharedDocumentRevision('1')),
		);
		await expect(completion).resolves.toEqual({ kind: 'completed' });
		expect(rendererRuntime.hasPublishedTerminal(mainResult)).toBe(true);
		expect(onRemoteTerminalCreated).toHaveBeenCalledWith(
			expect.objectContaining({ tabId: mainResult.tabId }),
		);
	});

	it('accepts only Main-confirmed precommit cancellation as cancelled', async () => {
		const runtime = createRemoteRuntime('cancelled');
		const { context, markCommitted } = createContext();

		await expect(
			executeWorkbenchTerminalCreationAction(
				context,
				{ groupId: null },
				{
					fallbackTitle: 'Remote Terminal',
					remoteTerminalCreation: capabilityReturning({ kind: 'cancelled' }),
					runtime,
				},
			),
		).resolves.toEqual({ kind: 'cancelled' });
		expect(markCommitted).not.toHaveBeenCalled();
	});

	it('does not relabel a committed result as cancellation while publication is pending', async () => {
		const rendererRuntime = createRemoteRuntime('postcommit-renderer');
		const mainRuntime = createRemoteRuntime('postcommit-main');
		const abortController = new AbortController();
		const { context, markCommitted } = createContext(abortController);
		const completion = Promise.resolve(
			executeWorkbenchTerminalCreationAction(
				context,
				{ groupId: null },
				{
					fallbackTitle: 'Remote Terminal',
					remoteTerminalCreation: capabilityReturning(createMainResult(mainRuntime)),
					runtime: rendererRuntime,
				},
			),
		);

		await Promise.resolve();
		abortController.abort();
		await expect(completion).resolves.toEqual({
			failure: { code: 'remote-terminal-publication-unavailable' },
			kind: 'failed',
		});
		expect(markCommitted).toHaveBeenCalledOnce();
	});

	it('maps bounded policy and operational failures without raw errors', async () => {
		const runtime = createRemoteRuntime('failure');
		const policyContext = createContext();
		const operationalContext = createContext();

		await expect(
			executeWorkbenchTerminalCreationAction(
				policyContext.context,
				{ groupId: null },
				{
					fallbackTitle: 'Remote Terminal',
					remoteTerminalCreation: capabilityReturning({
						kind: 'failed',
						reason: 'environment-not-connected',
					}),
					runtime,
				},
			),
		).resolves.toEqual({ code: 'environment-not-connected', kind: 'blocked' });
		await expect(
			executeWorkbenchTerminalCreationAction(
				operationalContext.context,
				{ groupId: null },
				{
					fallbackTitle: 'Remote Terminal',
					remoteTerminalCreation: capabilityReturning({
						kind: 'failed',
						reason: 'terminal-host-unavailable',
					}),
					runtime,
				},
			),
		).resolves.toEqual({
			failure: { code: 'terminal-host-unavailable' },
			kind: 'failed',
		});
	});
});
