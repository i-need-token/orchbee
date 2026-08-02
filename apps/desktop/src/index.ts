export {
	DesktopApplication,
	DesktopTerminalBindingRegistry,
	TerminalDocumentControlBroker,
	resolveDesktopRuntimeConfiguration,
} from './main';
export type {
	DesktopApplicationDependencies,
	DesktopApplicationOptions,
	DesktopRuntimeConfiguration,
	DesktopRuntimeConfigurationInput,
	DesktopRuntimeEnvironment,
	DesktopTerminalBindingRegistration,
	DesktopTerminalBindingRegistrationResult,
	DesktopTerminalBindingRemovalResult,
	TerminalDocumentControlOperations,
} from './main';
export { startDesktopRenderer } from './renderer';
export type { DesktopRendererDependencies, DesktopRendererSession } from './renderer';
