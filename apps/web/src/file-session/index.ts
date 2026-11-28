export { FileSessionRegistry } from './FileSessionRegistry';
export type { FileResource } from './file-resource';
export { filePathChild, filePathName, filePathParent, isFilePathWithin } from './file-paths';
export type {
	FileCapability,
	FileChange,
	FileRoot,
	FileRootEvent,
	FileSessionPort,
	FileWorkspaceSession,
} from './file-session-port';
export type { FileContent, FileContentPort, FileReadOptions } from './file-content-port';
export type { FilePathRules } from './file-resource';
export type { FileError, FileResult } from './file-error';
export type { FileGrantPort, FileGrantSource } from './file-grant-port';
export type { FileEntry, FileListPage, FileStatItem, FileTreePort } from './file-tree-port';
export type {
	GitMutation,
	GitMutationOutput,
	GitPort,
	GitPrompt,
	GitPromptAnswer,
	GitRelayEvent,
	GitRequestOutcome,
	GitSummary,
	GitSummaryEvent,
} from './git-port';
/** @internal The deterministic backend for tests; production sessions come from the wire adapters. */
export { MemoryFileSessionPort } from './memory';
