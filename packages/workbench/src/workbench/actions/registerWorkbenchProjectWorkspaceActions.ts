import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench-client-protocol';

import {
	deriveProjectNameFromPath,
	findWorkspaceResources,
	type ProjectWorkspaceEvent,
} from '../../project-workspace';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import { matchesWorkbenchProjectCreation } from '../model/matchesWorkbenchProjectCreation';
import type { WorkbenchNavigationPort } from '../types/workbench-host-actions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	workbenchProjectCreateInput,
	workbenchResourceDirectoryInput,
	workbenchResourceFolderAddInput,
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
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly primaryWorkspaceName: string;
	readonly defaultWorkspaceName: string;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
	readonly workspaceCopySuffix: string;
	readonly workspaceTerminalTitle: string;
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

function enteredFolderForPath(path: string) {
	return {
		available: true,
		git: null,
		id: `entered-folder:${path}`,
		kind: 'folder' as const,
		name: deriveProjectNameFromPath(path),
		path,
		placement: { kind: 'local' as const },
		source: 'selected' as const,
	};
}

export function registerWorkbenchProjectWorkspaceActions({
	navigation,
	primaryWorkspaceName,
	defaultWorkspaceName,
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
	workspaceCopySuffix,
	workspaceTerminalTitle,
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
					execute: (context, input) => {
						const directory = enteredFolderForPath(input.path);
						const transition = transitionWithCommit(context, projectWorkspaceRuntime, {
							directory,
							launchConfigurations: [],
							primaryWorkspaceLaunchConfigurationId: null,
							primaryWorkspaceName,
							projectName: directory.name,
							terminalTitle: workspaceTerminalTitle,
							type: 'project-created',
						});
						if (transition.outcome !== 'applied') return resultFor(transition);
						if (transition.projectCreationReceipt === undefined)
							throw new Error('Project creation requires its captured terminal receipt.');
						return { kind: 'completed', value: transition.projectCreationReceipt };
					},
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
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								beforeWorkspaceId: input.beforeWorkspaceId,

								type: 'workspace-reordered',
								workspaceId: input.workspaceId,
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.reorderWorkspace,
				input: workbenchWorkspaceReorderInput,
				title: workbenchMessages.workspaceReorder,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: (input) => input.resourceScopeId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) =>
					resourcesFor(input.resourceScopeId) === undefined
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				handler: {
					execute: (context, input) =>
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								directory: enteredFolderForPath(input.path),
								resourceScopeId: input.resourceScopeId,
								type: 'directory-added',
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.addResourceFolder,
				input: workbenchResourceFolderAddInput,
				title: workbenchMessages.projectAddFolder,
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
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								beforeProjectId: input.beforeProjectId,
								projectId: input.projectId,
								type: 'project-reordered',
							}),
						),
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
					const directory = resourcesFor(input.resourceScopeId)?.directories.find(
						(candidate) => candidate.id === input.directoryId,
					);
					return directory?.available === true
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable };
				},
				handler: {
					execute: (context, input) => {
						if (input === undefined)
							return resultFor(
								projectWorkspaceRuntime.createDefaultWorkspace(
									defaultWorkspaceName,
									workspaceTerminalTitle,
									context.markCommitted,
								),
							);
						const directory = resourcesFor(input.resourceScopeId)?.directories.find(
							(candidate) => candidate.id === input.directoryId,
						);
						if (directory === undefined) {
							return { code: 'directory-not-found', kind: 'blocked' };
						}
						if (directory.kind === 'worktree') {
							const associated = projectWorkspaceRuntime
								.getSnapshot()
								.workspaces.find((workspace) => {
									const primary = resourcesFor(workspace.resourceScopeId)?.directories.find(
										(candidate) => candidate.id === workspace.primaryDirectoryId,
									);
									return (
										primary?.kind === 'worktree' &&
										primary.placement.kind === 'local' &&
										directory.placement.kind === 'local' &&
										primary.path === directory.path &&
										primary.git.repositoryId === directory.git.repositoryId
									);
								});
							if (associated !== undefined)
								return navigation === undefined
									? resultFor(
											transitionWithCommit(context, projectWorkspaceRuntime, {
												type: 'workspace-activated',
												workspaceId: associated.id,
											}),
										)
									: navigation.navigate(context, { kind: 'workspace', workspaceId: associated.id });
						}
						return resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								directoryId: input.directoryId,
								resourceScopeId: input.resourceScopeId,
								launchConfigurationId: null,
								name: directory.name,
								projectId: input.projectId,
								terminalTitle: workspaceTerminalTitle,
								type: 'workspace-created',
							}),
						);
					},
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
				handler: {
					execute: (context, input) => {
						const workspace = workspaceFor(input);
						if (workspace === undefined) {
							return { code: 'workspace-not-found', kind: 'blocked' };
						}
						return resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								copyName: `${workspace.name} ${workspaceCopySuffix}`,

								type: 'workspace-duplicated',
								workspaceId: input.workspaceId,
							}),
						);
					},
				},
				id: WORKBENCH_ACTION_IDS.duplicateWorkspace,
				input: workbenchWorkspaceInput,
				title: workbenchMessages.workspaceDuplicate,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				confirmation: (input) => ({
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.workspaceDelete,
					description: {
						...workbenchMessages.workspaceDeleteDescription,
						values: { name: workspaceFor(input)?.name ?? '' },
					},
					risk: 'destructive',
					title: workbenchMessages.workspaceDeleteTitle,
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
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								type: 'workspace-deleted',
								workspaceId: input.workspaceId,
							}),
						),
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
				execution: projectExecution,
				getAvailability: (_context, input) =>
					projectFor(input.projectId) === undefined
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				handler: {
					execute: (context, input) =>
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								name: input.name,
								projectId: input.projectId,
								type: 'project-renamed',
							}),
						),
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
					concurrencyKey: (input) => input.resourceScopeId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) => {
					const project = resourcesFor(input.resourceScopeId);
					const directory = project?.directories.find(
						(candidate) => candidate.id === input.directoryId,
					);
					return project === undefined ||
						directory === undefined ||
						directory.kind === 'worktree' ||
						project.directories.length < 2
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' };
				},
				handler: {
					execute: (context, input) =>
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								directoryIds: [input.directoryId],
								resourceScopeId: input.resourceScopeId,
								type: 'directories-removed',
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.removeResourceDirectory,
				input: workbenchResourceDirectoryInput,
				title: workbenchMessages.projectRemoveFolder,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: (input) => input.resourceScopeId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) => {
					const project = resourcesFor(input.resourceScopeId);
					const directory = project?.directories.find(
						(candidate) => candidate.id === input.directoryId,
					);
					return directory?.available === true
						? {
								checked: project?.primaryDirectoryId === input.directoryId,
								kind: 'enabled',
							}
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable };
				},
				handler: {
					execute: (context, input) =>
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								directoryId: input.directoryId,
								resourceScopeId: input.resourceScopeId,
								type: 'resource-primary-directory-set',
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.setResourcePrimaryDirectory,
				input: workbenchResourceDirectoryInput,
				title: workbenchMessages.projectSetPrimaryFolder,
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
					const project = findWorkspaceResources(
						projectWorkspaceRuntime.getSnapshot(),
						input.workspaceId,
					);
					const directory = project?.directories.find(
						(candidate) => candidate.id === input.primaryDirectoryId,
					);
					return workspaceFor(input) !== undefined && directory?.available === true
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable };
				},
				handler: {
					execute: (context, input) => {
						const workspace = workspaceFor(input);
						if (workspace === undefined) {
							return { code: 'workspace-not-found', kind: 'blocked' };
						}
						return resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								launchConfigurationId: workspace.launchConfigurationId,
								name: input.name,
								primaryDirectoryId: input.primaryDirectoryId,

								type: 'workspace-updated',
								workspaceId: input.workspaceId,
							}),
						);
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
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								projectId: input.projectId,
								type: 'project-dissolved',
							}),
						),
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
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								...input,
								type: 'workspace-project-set',
							}),
						),
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
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								...input,
								type: 'workspace-project-created',
							}),
						),
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
					projectFor(input.projectId) !== undefined
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable },
				confirmation: (input) => ({
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.projectCloseAll,
					description: {
						...workbenchMessages.projectCloseAllDescription,
						values: { name: projectFor(input.projectId)?.name ?? '' },
					},
					risk: 'destructive',
					title: workbenchMessages.projectCloseAll,
				}),
				handler: {
					execute: (context, input) =>
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								...input,
								type: 'project-workspaces-closed',
							}),
						),
				},
			}),
		),
	];
}
