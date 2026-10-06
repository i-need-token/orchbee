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
export {
	composeDesktopTerminalLifecycleAdapter,
	DesktopTerminalLifecycleAdapter,
} from './terminal-document-control';
export type {
	DesktopTerminalBindingLifecyclePort,
	DesktopTerminalLifecycleAdapterComposition,
	DesktopTerminalLifecycleAdapterDependencies,
	DesktopTerminalLifecycleOperations,
} from './terminal-document-control';
