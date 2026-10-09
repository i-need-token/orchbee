import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import { deriveProjectNameFromPath, type ProjectWorkspaceEvent } from '../../project-workspace';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import { matchesWorkbenchProjectCreation } from '../model/matchesWorkbenchProjectCreation';
import { workbenchSharedDocumentRevision } from '../document';
import type { WorkbenchNavigationPort } from '../types/workbench-host-actions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	workbenchProjectCreateInput,
	workbenchProjectDirectoryInput,
	workbenchProjectFolderAddInput,
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
} from './workbench-action-ids';

interface RegisterWorkbenchProjectWorkspaceActionsOptions {
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly primaryWorkspaceName: string;
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
		projectFor(input.projectId)?.workspaces.find((workspace) => workspace.id === input.workspaceId);
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
					concurrencyKey: (input) => `workspace-order:${input.projectId}`,
					mode: 'foreground',
				},
				getAvailability: (_context, input) => {
					const project = projectFor(input.projectId);
					return project !== undefined &&
						project.workspaces.length > 1 &&
						project.workspaces.some((workspace) => workspace.id === input.workspaceId) &&
						(input.beforeWorkspaceId === null ||
							project.workspaces.some((workspace) => workspace.id === input.beforeWorkspaceId))
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable };
				},
				handler: {
					execute: (context, input) =>
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								beforeWorkspaceId: input.beforeWorkspaceId,
								projectId: input.projectId,
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
				execution: projectExecution,
				getAvailability: (_context, input) =>
					projectFor(input.projectId) === undefined
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				handler: {
					execute: (context, input) =>
						resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								directory: enteredFolderForPath(input.path),
								projectId: input.projectId,
								type: 'directory-added',
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.addProjectFolder,
				input: workbenchProjectFolderAddInput,
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
					concurrencyKey: (input) => `${input.projectId}:${input.workspaceId}`,
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
									activeLocation.projectId === input.projectId &&
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
						return navigation === undefined
							? resultFor(
									transitionWithCommit(context, projectWorkspaceRuntime, {
										projectId: input.projectId,
										type: 'workspace-activated',
										workspaceId: input.workspaceId,
									}),
								)
							: navigation.navigate(context, {
									kind: 'workspace',
									projectId: input.projectId,
									workspaceId: input.workspaceId,
								});
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
					concurrencyKey: (input) => input.projectId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) => {
					const directory = projectFor(input.projectId)?.directories.find(
						(candidate) => candidate.id === input.directoryId,
					);
					return directory?.available === true
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: workbenchMessages.actionUnavailable };
				},
				handler: {
					execute: (context, input) => {
						const directory = projectFor(input.projectId)?.directories.find(
							(candidate) => candidate.id === input.directoryId,
						);
						if (directory === undefined) {
							return { code: 'directory-not-found', kind: 'blocked' };
						}
						return resultFor(
							transitionWithCommit(context, projectWorkspaceRuntime, {
								directoryId: input.directoryId,
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
					concurrencyKey: (input) => input.projectId,
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
								projectId: input.projectId,
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
					concurrencyKey: (input) => input.projectId,
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
								projectId: input.projectId,
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
				execution: projectExecution,
				getAvailability: (_context, input) => {
					const project = projectFor(input.projectId);
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
								projectId: input.projectId,
								type: 'directories-removed',
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.removeProjectDirectory,
				input: workbenchProjectDirectoryInput,
				title: workbenchMessages.projectRemoveFolder,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: projectExecution,
				getAvailability: (_context, input) => {
					const project = projectFor(input.projectId);
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
								projectId: input.projectId,
								type: 'project-primary-directory-set',
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.setProjectPrimaryDirectory,
				input: workbenchProjectDirectoryInput,
				title: workbenchMessages.projectSetPrimaryFolder,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: categoryProjects,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: (input) => input.projectId,
					mode: 'foreground',
				},
				getAvailability: (_context, input) => {
					const project = projectFor(input.projectId);
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
								projectId: input.projectId,
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
				confirmation: (input) => ({
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.projectRemove,
					description: {
						...workbenchMessages.projectRemoveDescription,
						values: { name: projectFor(input.projectId)?.name ?? '' },
					},
					risk: 'destructive',
					title: workbenchMessages.projectRemoveTitle,
				}),
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
								type: 'project-removed',
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.removeProject,
				input: workbenchProjectInput,
				title: workbenchMessages.projectRemove,
			}),
		),
	];
}
