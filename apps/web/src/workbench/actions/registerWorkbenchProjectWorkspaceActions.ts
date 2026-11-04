import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionResult,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench-client-protocol';

import { deriveProjectNameFromPath, type ProjectWorkspaceEvent } from '../../project-workspace';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import { matchesWorkbenchProjectCreation } from '../model/matchesWorkbenchProjectCreation';
import { workbenchActiveLocationsEqual } from '../model/workbench-terminal-location-activation';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityClosure,
	WorkbenchAuthorityCreation,
	WorkbenchAuthorityNavigationChange,
} from '../types/workbench-authority';
import type { WorkbenchFocusRouting } from '../types/workbench-focus-routing';
import type { WorkbenchNavigationPort } from '../types/workbench-host-actions';
import type { WorkbenchProjectCreationReceipt } from '../types/workbench-project-folder-acquisition';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	workbenchProjectCreateInput,
	type WorkbenchProjectInput,
	workbenchProjectInput,
	workbenchProjectRenameInput,
	workbenchProjectReorderInput,
	type WorkbenchWorkspaceInput,
	workbenchWorkspaceCreateInput,
	workbenchWorkspaceInput,
	workbenchWorkspaceActivationInput,
	workbenchWorkspaceReorderInput,
	workbenchWorkspaceUpdateInput,
	workbenchWorkspaceProjectInput,
	workbenchWorkspaceProjectCreateInput,
} from './workbench-action-ids';

interface RegisterWorkbenchProjectWorkspaceActionsOptions {
	/** Decides the shared Workbench structure. */
	readonly authority: WorkbenchAuthority;
	readonly beginTerminalHandover?: WorkbenchFocusRouting['beginTerminalHandover'] | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

function resultFor(transition: ReturnType<WorkbenchProjectWorkspaceRuntime['transition']>) {
	if (transition.outcome === 'rejected') {
		return {
			code: transition.rejection ?? 'project-workspace-transition-rejected',
			kind: 'blocked' as const,
		};
	}
	if (transition.outcome === 'unchanged') return { kind: 'unchanged' as const };
	return { kind: 'completed' as const };
}

function transitionWithCommit(
	context: ActionExecutionContext,
	runtime: WorkbenchProjectWorkspaceRuntime,
	event: ProjectWorkspaceEvent,
) {
	return runtime.transition(event, context.markCommitted);
}

function withoutReceipt(result: ActionResult<WorkbenchProjectCreationReceipt>): ActionResult<void> {
	return result.kind === 'completed' || result.kind === 'unchanged'
		? { kind: result.kind }
		: result;
}

export function registerWorkbenchProjectWorkspaceActions({
	authority,
	beginTerminalHandover,
	navigation,
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
}: RegisterWorkbenchProjectWorkspaceActionsOptions): readonly ActionDisposable[] {
	const categoryNavigation = {
		id: 'navigation',
		title: workbenchMessages.actionCategoryNavigation,
	};
	const categoryProjects = {
		id: 'projects',
		title: workbenchMessages.actionCategoryProjects,
	};
	const projectFor = (projectId: string) =>
		projectWorkspaceRuntime.getSnapshot().projects.find((project) => project.id === projectId);
	const workspaceFor = (input: WorkbenchWorkspaceInput) =>
		projectWorkspaceRuntime
			.getSnapshot()
			.workspaces.find((workspace) => workspace.id === input.workspaceId);
	const resourcesFor = (resourceScopeId: string) =>
		projectWorkspaceRuntime
			.getSnapshot()
			.resourceScopes.find((resource) => resource.id === resourceScopeId);
	const projectExecution = {
		concurrency: 'serialize' as const,
		concurrencyKey: (input: WorkbenchProjectInput) => input.projectId,
		mode: 'foreground' as const,
	};
	const commit = (context: ActionExecutionContext, change: WorkbenchAuthorityNavigationChange) =>
		authority.change(context, change);
	const enterWorkspace = (context: ActionExecutionContext, workspaceId: string) =>
		navigation === undefined
			? resultFor(
					transitionWithCommit(context, projectWorkspaceRuntime, {
						type: 'workspace-activated',
						workspaceId,
					}),
				)
			: navigation.navigate(context, { kind: 'workspace', workspaceId });
	const createAndEnter = async (
		context: ActionExecutionContext,
		creation: WorkbenchAuthorityCreation,
	): Promise<ActionResult<WorkbenchProjectCreationReceipt>> => {
		const origin = projectWorkspaceRuntime.getSnapshot().activeLocation;
		const handover = beginTerminalHandover?.();
		const created = await authority.create(context, creation);
		if (created.kind !== 'completed' || created.value === undefined) return created;
		// Late completion never takes someone from where they have since gone, or from what they did next.
		if (
			context.signal.aborted ||
			handover?.held() === false ||
			workspaceFor(created.value) === undefined
		) {
			return created;
		}
		if (
			workbenchActiveLocationsEqual(origin, projectWorkspaceRuntime.getSnapshot().activeLocation)
		) {
			await enterWorkspace(context, created.value.workspaceId);
		}
		// The first Workspace has already replaced the empty location, so this asks what is shown now.
		const location = projectWorkspaceRuntime.getSnapshot().activeLocation;
		if (location.kind === 'workspace' && location.workspaceId === created.value.workspaceId) {
			handover?.complete();
		}
		return created;
	};
	/** What closed was what the person acted from, so the keyboard goes to the terminal still shown. */
	const closeAndReturnToTerminal = async (
		context: ActionExecutionContext,
		closure: WorkbenchAuthorityClosure,
	): Promise<ActionResult<void>> => {
		const handover = beginTerminalHandover?.();
		const result = await authority.close(context, closure);
		if (result.kind === 'completed') handover?.complete();
		return result;
	};
	const workspaceCloseConfirmation = (input: WorkbenchWorkspaceInput) => ({
		cancelLabel: workbenchMessages.panelCancel,
		confirmLabel: workbenchMessages.panelClose,
		description: {
			...workbenchMessages.workspaceDeleteDescription,
			values: { name: workspaceFor(input)?.name ?? '' },
		},
		risk: 'destructive' as const,
		title: workbenchMessages.workspaceDeleteTitle,
	});

	return [
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: () => 'project-creation',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (context, input) =>
						createAndEnter(context, {
							path: input.path,
							projectName: deriveProjectNameFromPath(input.path),
							type: 'project-created',
						}),
				},
				id: WORKBENCH_ACTION_IDS.createProject,
				input: workbenchProjectCreateInput,
				title: workbenchMessages.projectCreate,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: (input) => `workspace-order:${workspaceFor(input)?.projectId}`,
					mode: 'foreground',
				},
				getAvailability: (_context, input) => {
					const workspace = workspaceFor(input);
					const members = projectWorkspaceRuntime
						.getSnapshot()
						.workspaces.filter((candidate) => candidate.projectId === workspace?.projectId);
					return workspace !== undefined &&
						members.length > 1 &&
						(input.beforeWorkspaceId === null ||
							members.some((member) => member.id === input.beforeWorkspaceId))
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable };
				},
				handler: {
					execute: (context, input) =>
						commit(context, {
							beforeWorkspaceId: input.beforeWorkspaceId,
							type: 'workspace-reordered',
							workspaceId: input.workspaceId,
						}),
				},
				id: WORKBENCH_ACTION_IDS.reorderWorkspace,
				input: workbenchWorkspaceReorderInput,
				title: workbenchMessages.workspaceReorder,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryNavigation,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'workbench',
					mode: 'foreground',
				},
				getAvailability: () => ({
					checked: projectWorkspaceRuntime.getSnapshot().activeLocation.kind === 'standalone',
					kind: 'enabled',
				}),
				handler: {
					execute: (context) =>
						navigation === undefined
							? resultFor(
									transitionWithCommit(context, projectWorkspaceRuntime, {
										type: 'standalone-activated',
									}),
								)
							: navigation.navigate(context, { kind: 'standalone' }),
				},
				id: WORKBENCH_ACTION_IDS.activateStandalone,
				input: noActionInput,
				title: workbenchMessages.terminal,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: () => 'project-order',
					mode: 'foreground',
				},
				getAvailability: (_context, input) =>
					projectWorkspaceRuntime.getSnapshot().projects.length > 1 &&
					projectFor(input.projectId) !== undefined &&
					(input.beforeProjectId === null || projectFor(input.beforeProjectId) !== undefined)
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable },
				handler: {
					execute: (context, input) =>
						commit(context, {
							beforeProjectId: input.beforeProjectId,
							projectId: input.projectId,
							type: 'project-reordered',
						}),
				},
				id: WORKBENCH_ACTION_IDS.reorderProject,
				input: workbenchProjectReorderInput,
				title: workbenchMessages.projectReorder,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: projectExecution,
				getAvailability: (_context, input) =>
					projectFor(input.projectId) === undefined
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: {
								checked: projectWorkspaceRuntime
									.getSnapshot()
									.expandedProjectIds.includes(input.projectId),
								kind: 'enabled',
							},
				handler: {
					execute: (context, input) =>
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								projectId: input.projectId,
								type: 'project-expansion-toggled',
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.toggleProjectExpansion,
				input: workbenchProjectInput,
				title: workbenchMessages.projectToggleExpansion,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: () => 'recents',
					mode: 'foreground',
				},
				getAvailability: () => ({
					checked: projectWorkspaceRuntime.getSnapshot().recentsExpanded,
					kind: 'enabled',
				}),
				handler: {
					execute: (context) =>
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								type: 'recents-expansion-toggled',
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.toggleRecentsExpansion,
				input: noActionInput,
				title: workbenchMessages.recentsToggleExpansion,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: (input) => input.workspaceId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) => {
					const activeLocation = projectWorkspaceRuntime.getSnapshot().activeLocation;
					return workspaceFor(input) === undefined ||
						(input.creationReceipt !== undefined &&
							!matchesWorkbenchProjectCreation(
								projectWorkspaceRuntime.getSharedDocument(workbenchSharedDocumentRevision('0')),
								input.creationReceipt,
							))
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: {
								checked:
									activeLocation.kind === 'workspace' &&
									activeLocation.workspaceId === input.workspaceId,
								kind: 'enabled',
							};
				},
				handler: {
					execute: (context, input) => {
						if (
							input.creationReceipt !== undefined &&
							!matchesWorkbenchProjectCreation(
								projectWorkspaceRuntime.getSharedDocument(workbenchSharedDocumentRevision('0')),
								input.creationReceipt,
							)
						)
							return { code: 'project-creation-target-replaced', kind: 'blocked' };
						if (navigation === undefined)
							return resultFor(
								transitionWithCommit(context, projectWorkspaceRuntime, {
									type: 'workspace-activated',
									workspaceId: input.workspaceId,
								}),
							);
						const target = {
							kind: 'workspace' as const,

							workspaceId: input.workspaceId,
						};
						return input.creationReceipt === undefined
							? navigation.navigate(context, target)
							: navigation.navigate(context, target, input.creationReceipt);
					},
				},
				id: WORKBENCH_ACTION_IDS.activateWorkspace,
				input: workbenchWorkspaceActivationInput,
				title: workbenchMessages.workspaceActivate,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: () => 'workspace-creation',
					mode: 'foreground',
				},
				getAvailability: (_context, input) => {
					if (input === undefined) return { kind: 'enabled' };
					if (!('directoryId' in input)) {
						return projectFor(input.projectId) === undefined
							? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
							: { kind: 'enabled' };
					}
					const directory = resourcesFor(input.resourceScopeId)?.directories.find(
						(candidate) => candidate.id === input.directoryId,
					);
					return directory?.available === true
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable };
				},
				handler: {
					execute: (context, input) =>
						createAndEnter(
							context,
							input === undefined || !('directoryId' in input)
								? { projectId: input?.projectId ?? null, type: 'workspace-created-default' }
								: {
										directoryId: input.directoryId,
										projectId: input.projectId,
										type: 'workspace-created',
									},
						).then(withoutReceipt),
				},
				id: WORKBENCH_ACTION_IDS.createWorkspace,
				input: workbenchWorkspaceCreateInput,
				title: workbenchMessages.projectCreateWorkspace,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: (input) => input.workspaceId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) =>
					workspaceFor(input) === undefined
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				// A Workspace that runs nothing closes without asking, and only the authority can tell.
				handler: {
					execute: async (context, input) => {
						const result = await closeAndReturnToTerminal(context, {
							type: 'workspace-close-if-idle',
							workspaceId: input.workspaceId,
						});
						if (result.kind !== 'blocked' || result.code !== 'confirmation-required') {
							return result;
						}
						// The question is its own Action, because this one has already asked the authority.
						typedRuntime.execute(WORKBENCH_ACTION_IDS.closeWorkspaceConfirmed, input, {
							scope,
							source: context.source,
						});
						return { kind: 'unchanged' };
					},
				},
				id: WORKBENCH_ACTION_IDS.deleteWorkspace,
				input: workbenchWorkspaceInput,
				title: workbenchMessages.workspaceDelete,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				confirmation: (input) => ({
					...workspaceCloseConfirmation(input),
					watch: (withdraw) =>
						authority.watchClose(
							{ type: 'workspace-close', workspaceId: input.workspaceId },
							withdraw,
						),
				}),
				execution: {
					concurrency: 'serialize',
					concurrencyKey: (input) => input.workspaceId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) =>
					workspaceFor(input) === undefined
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				handler: {
					execute: (context, input) =>
						closeAndReturnToTerminal(context, {
							type: 'workspace-close',
							workspaceId: input.workspaceId,
						}),
				},
				id: WORKBENCH_ACTION_IDS.closeWorkspaceConfirmed,
				input: workbenchWorkspaceInput,
				title: workbenchMessages.workspaceDelete,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: projectExecution,
				getAvailability: (_context, input) =>
					projectFor(input.projectId) === undefined
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				handler: {
					execute: (context, input) =>
						commit(context, {
							name: input.name,
							projectId: input.projectId,
							type: 'project-renamed',
						}),
				},
				id: WORKBENCH_ACTION_IDS.renameProject,
				input: workbenchProjectRenameInput,
				title: workbenchMessages.projectManage,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: (input) => input.workspaceId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) =>
					workspaceFor(input) === undefined
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				handler: {
					execute: (context, input) => {
						const workspace = workspaceFor(input);
						if (workspace === undefined) {
							return { code: 'workspace-not-found', kind: 'blocked' };
						}
						return commit(context, {
							launchConfigurationId: workspace.launchConfigurationId,
							name: input.name,
							primaryDirectoryId: input.primaryDirectoryId,
							type: 'workspace-updated',
							workspaceId: input.workspaceId,
						});
					},
				},
				id: WORKBENCH_ACTION_IDS.updateWorkspace,
				input: workbenchWorkspaceUpdateInput,
				title: workbenchMessages.workspaceManage,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: projectExecution,
				getAvailability: (_context, input) =>
					projectFor(input.projectId) === undefined
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				handler: {
					execute: (context, input) =>
						commit(context, {
							projectId: input.projectId,
							type: 'project-dissolved',
						}),
				},
				id: WORKBENCH_ACTION_IDS.removeProject,
				input: workbenchProjectInput,
				title: workbenchMessages.projectRemove,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				id: WORKBENCH_ACTION_IDS.setWorkspaceProject,
				title: workbenchMessages.workspaceMoveToProject,
				input: workbenchWorkspaceProjectInput,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: (input) => input.workspaceId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) =>
					workspaceFor(input) !== undefined &&
					(input.projectId === null || projectFor(input.projectId) !== undefined)
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable },
				handler: {
					execute: (context, input) =>
						commit(context, {
							...input,
							type: 'workspace-project-set',
						}),
				},
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				id: WORKBENCH_ACTION_IDS.createWorkspaceProject,
				title: workbenchMessages.workspaceCreateProject,
				input: workbenchWorkspaceProjectCreateInput,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: (input) => input.workspaceId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) =>
					workspaceFor(input) !== undefined
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable },
				handler: {
					execute: (context, input) =>
						commit(context, {
							...input,
							type: 'workspace-project-created',
						}),
				},
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				id: WORKBENCH_ACTION_IDS.closeProjectWorkspaces,
				title: workbenchMessages.projectCloseAll,
				input: workbenchProjectInput,
				execution: projectExecution,
				getAvailability: (_context, input) =>
					projectFor(input.projectId) === undefined
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				confirmation: (input) => ({
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.panelClose,
					description: {
						...workbenchMessages.projectCloseAllDescription,
						values: { name: projectFor(input.projectId)?.name ?? '' },
					},
					risk: 'destructive',
					title: {
						...workbenchMessages.projectCloseAllTitle,
						values: {
							count: projectWorkspaceRuntime
								.getSnapshot()
								.workspaces.filter((workspace) => workspace.projectId === input.projectId).length,
						},
					},
					watch: (withdraw) =>
						authority.watchClose(
							{ projectId: input.projectId, type: 'project-workspaces-close' },
							withdraw,
						),
				}),
				handler: {
					execute: (context, input) =>
						closeAndReturnToTerminal(context, {
							projectId: input.projectId,
							type: 'project-workspaces-close',
						}),
				},
			}),
		),
	];
}
