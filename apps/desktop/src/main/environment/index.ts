export {
	EnvironmentIdSchema,
	EnvironmentRecordSchema,
	sshEnvironmentTargetKey,
	type EnvironmentId,
	type EnvironmentRecord,
} from './environment-registry-contract';
export { DesktopEnvironmentRegistry } from './DesktopEnvironmentRegistry';
export type {
	EnvironmentRegistryCreateResult,
	EnvironmentRegistryInitializeResult,
	EnvironmentRegistryReadResult,
} from './DesktopEnvironmentRegistry';
export { FileEnvironmentRegistryStore } from './FileEnvironmentRegistryStore';
export { validateWorkbenchEnvironmentReferences } from './validateWorkbenchEnvironmentReferences';
