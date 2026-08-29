import { z } from 'zod';

import {
	TerminalPresentationIdSchema,
	TerminalPresentationOperationIdSchema,
	TerminalPresentationRemovalIdSchema,
} from '../../terminal-presentation';
import { WorkbenchSharedDocumentRevisionSchema } from '../document';

const identifierSchema = z.string().min(1).max(512);
const optionalPageShape = {
	cursor: z.string().min(1).max(4096).optional(),
	limit: z.number().int().min(1).max(200).default(100),
};

export const WorkbenchProjectListInputSchema = z.strictObject(optionalPageShape);
export const WorkbenchDirectoryListInputSchema = z.strictObject({
	projectId: identifierSchema,
	...optionalPageShape,
});
export const WorkbenchWorkspaceListInputSchema = z.strictObject({
	projectId: identifierSchema,
	...optionalPageShape,
});

export const WorkbenchLocationRefSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('standalone') }),
	z.strictObject({
		kind: z.literal('workspace'),
		projectId: identifierSchema,
		workspaceId: identifierSchema,
	}),
]);

export const WorkbenchTerminalListInputSchema = z.strictObject({
	location: WorkbenchLocationRefSchema,
	...optionalPageShape,
});

const pageShape = {
	nextCursor: z.string().min(1).max(4096).nullable(),
	revision: WorkbenchSharedDocumentRevisionSchema,
};

const projectListItemSchema = z.strictObject({
	name: z.string(),
	projectId: identifierSchema,
});

const directoryListItemSchema = z.strictObject({
	available: z.boolean(),
	directoryId: identifierSchema,
	kind: z.enum(['folder', 'worktree']),
	name: z.string(),
	path: z.string().min(1),
	primary: z.boolean(),
	source: z.enum(['created', 'discovered', 'selected']),
});

const workspaceListItemSchema = z.strictObject({
	name: z.string(),
	primaryDirectoryId: identifierSchema,
	workspaceId: identifierSchema,
});

const terminalIntentSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('provision'),
		operationId: TerminalPresentationOperationIdSchema,
	}),
	z.strictObject({ kind: z.literal('retain') }),
	z.strictObject({
		kind: z.literal('stop'),
		operationId: TerminalPresentationOperationIdSchema,
	}),
	z.strictObject({ kind: z.literal('stopped') }),
	z.strictObject({
		kind: z.literal('close'),
		operationId: TerminalPresentationOperationIdSchema,
		removalId: TerminalPresentationRemovalIdSchema,
	}),
]);

const terminalListItemSchema = z.strictObject({
	groupId: identifierSchema.nullable(),
	intent: terminalIntentSchema,
	launchCwd: z.string().nullable(),
	launchDirectoryId: identifierSchema.nullable(),
	panelId: identifierSchema,
	pinned: z.boolean(),
	presentationId: TerminalPresentationIdSchema,
	role: z.enum(['child', 'primary']),
	tabCustomTitle: z.string().min(1).nullable(),
	tabFallbackTitle: z.string(),
	tabId: identifierSchema,
	terminalContextId: identifierSchema,
	terminalTitle: z.string(),
});

export const WorkbenchProjectListPageSchema = z.strictObject({
	...pageShape,
	items: z.array(projectListItemSchema).max(200),
});
export const WorkbenchDirectoryListPageSchema = z.strictObject({
	...pageShape,
	items: z.array(directoryListItemSchema).max(200),
});
export const WorkbenchWorkspaceListPageSchema = z.strictObject({
	...pageShape,
	items: z.array(workspaceListItemSchema).max(200),
});
export const WorkbenchTerminalListPageSchema = z.strictObject({
	...pageShape,
	items: z.array(terminalListItemSchema).max(200),
});

export const WORKBENCH_PROJECT_LIST_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'workbench.project.list',
	input: { name: 'workbench.project-list-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'workbench.project-list-page', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export const WORKBENCH_DIRECTORY_LIST_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'workbench.directory.list',
	input: { name: 'workbench.directory-list-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'workbench.directory-list-page', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export const WORKBENCH_WORKSPACE_LIST_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'workbench.workspace.list',
	input: { name: 'workbench.workspace-list-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'workbench.workspace-list-page', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export const WORKBENCH_TERMINAL_LIST_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'workbench.terminal.list',
	input: { name: 'workbench.terminal-list-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'workbench.terminal-list-page', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export type WorkbenchPageInput = z.infer<typeof WorkbenchProjectListInputSchema>;
export type WorkbenchProjectListInput = z.infer<typeof WorkbenchProjectListInputSchema>;
export type WorkbenchDirectoryListInput = z.infer<typeof WorkbenchDirectoryListInputSchema>;
export type WorkbenchWorkspaceListInput = z.infer<typeof WorkbenchWorkspaceListInputSchema>;
export type WorkbenchTerminalListInput = z.infer<typeof WorkbenchTerminalListInputSchema>;
export type WorkbenchProjectListPage = z.infer<typeof WorkbenchProjectListPageSchema>;
export type WorkbenchDirectoryListPage = z.infer<typeof WorkbenchDirectoryListPageSchema>;
export type WorkbenchWorkspaceListPage = z.infer<typeof WorkbenchWorkspaceListPageSchema>;
export type WorkbenchTerminalListPage = z.infer<typeof WorkbenchTerminalListPageSchema>;
