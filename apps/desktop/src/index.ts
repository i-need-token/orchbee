export {
	DesktopApplication,
	DesktopTerminalBindingRegistry,
	DesktopTerminalLifecycleAdapter,
	TerminalDocumentControlBroker,
	composeDesktopTerminalLifecycleAdapter,
	resolveDesktopRuntimeConfiguration,
} from './main';
export type {
	DesktopApplicationDependencies,
	DesktopApplicationOptions,
	DesktopRuntimeConfiguration,
	DesktopRuntimeConfigurationInput,
	DesktopRuntimeEnvironment,
	DesktopTerminalBindingLifecyclePort,
	DesktopTerminalBindingRegistration,
	DesktopTerminalBindingRegistrationResult,
	DesktopTerminalBindingRemovalResult,
	DesktopTerminalLifecycleAdapterComposition,
	DesktopTerminalLifecycleAdapterDependencies,
	TerminalDocumentControlOperations,
} from './main';
export { startDesktopRenderer } from './renderer';
export type { DesktopRendererDependencies, DesktopRendererSession } from './renderer';
