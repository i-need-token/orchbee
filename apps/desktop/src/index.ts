export {
	DesktopApplication,
	DesktopTerminalBindingRegistry,
	DesktopTerminalLifecycleAdapter,
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
	DesktopTerminalLifecycleOperations,
} from './main';
export { startDesktopRenderer, startDesktopWorkbenchRenderer } from './renderer';
export type {
	DesktopRendererDependencies,
	DesktopRendererSession,
	DesktopWorkbenchRendererSession,
} from './renderer';
