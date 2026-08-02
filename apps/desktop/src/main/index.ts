export { DesktopApplication } from './composition';
export type { DesktopApplicationDependencies, DesktopApplicationOptions } from './composition';
export {
	resolveDesktopRuntimeConfiguration,
	type DesktopRuntimeConfiguration,
	type DesktopRuntimeConfigurationInput,
	type DesktopRuntimeEnvironment,
} from './composition';
export { DesktopTerminalBindingRegistry } from './terminal-binding-registry';
export type {
	DesktopTerminalBindingRegistration,
	DesktopTerminalBindingRegistrationResult,
	DesktopTerminalBindingRemovalResult,
} from './terminal-binding-registry';
export { TerminalDocumentControlBroker } from './terminal-document-control';
export type { TerminalDocumentControlOperations } from './terminal-document-control';
