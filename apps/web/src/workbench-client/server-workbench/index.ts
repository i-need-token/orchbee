export { ServerAgentIntegrationClient } from './ServerAgentIntegrationClient';
export { ServerTerminalDefaultSettingsClient } from './ServerTerminalDefaultSettingsClient';
export { ServerTerminalLaunchProfileClient } from './ServerTerminalLaunchProfileClient';
export { ServerTerminalProjectionClient } from './ServerTerminalProjectionClient';
export { ServerUserPreferencesClient } from './ServerUserPreferencesClient';
export { ServerWorkbenchSession } from './ServerWorkbenchSession';
export { createServerWorkbenchAuthority } from './createServerWorkbenchAuthority';
export { findServerPresentation } from './findServerPresentation';
export { findServerTerminalBinding } from './findServerTerminalBinding';
export { projectServerSnapshot } from './projectServerSnapshot';
export {
	createServerTerminalColorsSource,
	presentServerTerminalColors,
	serverHostColors,
} from './server-terminal-colors';
export type { ServerTerminalColorsSource } from './server-terminal-colors';
export { nameServerTerminalTitle } from './server-terminal-names';
export { admitsAutomaticWorkspace, workspaceLaunchAction } from './workspace-launch';
