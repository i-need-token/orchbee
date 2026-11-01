export { WorkbenchClientSessionClient } from './client-session';
export type { WorkbenchClientSessionTransport } from './client-session';
export { createWorkbenchClientRuntime } from './composition';
export type { WorkbenchClientRuntime } from './composition';
export type {
	ProjectedTerminalTransport,
	TerminalProjection,
	TerminalProjectionClient,
	TerminalProjectionRequest,
} from './terminal-projection';
export type { ProjectedTerminalPlatformEffects } from './projected-terminal-platform-effects';
export {
	registerTerminalProductFonts,
	startProjectedTerminalRenderer,
} from './projected-terminal-renderer';
export {
	type ServerCarrier,
	ServerConnection,
	type ServerTerminalTarget,
	createServerTerminal,
	openServerTerminalProjection,
} from './server-connection';
export { ProjectedTerminalSurface } from './projected-terminal-surface';
export { WorkbenchClientApplication } from './workbench-client-application';
export type { WorkbenchClientApplicationProps } from './workbench-client-application';
export { WorkbenchDocumentCoordinator } from './workbench-document-control';
