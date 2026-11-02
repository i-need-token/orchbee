export { WorkbenchClientSessionClient } from './client-session';
export type { WorkbenchClientSessionTransport } from './client-session';
export { createServerWorkbenchRuntime, createWorkbenchClientRuntime } from './composition';
export type { ServerWorkbenchRuntime, WorkbenchClientRuntime } from './composition';
export type {
	ProjectedTerminalTransport,
	TerminalProjection,
	TerminalProjectionClient,
	TerminalProjectionRequest,
} from './terminal-projection';
export type { ProjectedTerminalPlatformEffects } from './projected-terminal-platform-effects';
export { registerTerminalProductFonts } from './projected-terminal-renderer';
export { type ServerCarrier, ServerConnection } from './server-connection';
export {
	admitsAutomaticWorkspace,
	ServerUserPreferencesClient,
	ServerWorkbenchSession,
	workspaceLaunchAction,
} from './server-workbench';
export { ProjectedTerminalSurface } from './projected-terminal-surface';
export { WorkbenchClientApplication } from './workbench-client-application';
export type { WorkbenchClientApplicationProps } from './workbench-client-application';
export { WorkbenchDocumentCoordinator } from './workbench-document-control';
