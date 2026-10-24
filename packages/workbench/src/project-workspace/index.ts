export {
	assertProjectWorkspaceInvariants,
	createProjectWorkspaceState,
	findWorkbenchWorkspace,
	findWorkspaceResources,
	transitionProjectWorkspace,
} from './project-workspace-state';
export { deriveProjectNameFromPath } from './deriveProjectNameFromPath';
export type {
	ProjectWorkspaceEvent,
	ProjectWorkspaceState,
	ProjectWorkspaceTransition,
	TerminalContextCommand,
	WorkbenchActiveLocation,
	WorkbenchDirectory,
	WorkbenchDirectoryPlacement,
	WorkbenchFolderDirectory,
	WorkbenchProject,
	WorkbenchResourceScope,
	WorkbenchWorkspace,
} from './project-workspace-state';
export { resolveTerminalCwd } from './resolveTerminalCwd';
export type { ResolveTerminalCwdResult } from './resolveTerminalCwd';
