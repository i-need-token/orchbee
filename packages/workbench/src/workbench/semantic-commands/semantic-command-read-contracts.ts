import { z } from 'zod';

import {
	TerminalPresentationIdSchema,
	TerminalPresentationOperationIdSchema,
	TerminalPresentationRemovalIdSchema,
} from '../../terminal-presentation';
import { WorkbenchSharedDocumentRevisionSchema } from '../document';

const identifierSchema = z.string().min(1).max(512);
const textEncoder = new TextEncoder();
const maximumUint64Text = '18446744073709551615';
const terminalUint64TextSchema = z
	.string()
	.regex(/^(?:0|[1-9][0-9]{0,19})$/u)
	.refine(
		(value) =>
			value.length < maximumUint64Text.length ||
			(value.length === maximumUint64Text.length && value <= maximumUint64Text),
		'Terminal value exceeds unsigned 64-bit range',
	);
const optionalPageShape = {
	cursor: z.string().min(1).max(4096).optional(),
	limit: z.number().int().min(1).max(200).default(100),
};

export const WorkbenchProjectListInputSchema = z.strictObject(optionalPageShape);
export const WorkbenchDirectoryListInputSchema = z.strictObject({
	resourceScopeId: identifierSchema,
	...optionalPageShape,
});
export const WorkbenchWorkspaceListInputSchema = z.strictObject({
	projectId: identifierSchema.nullable(),
	...optionalPageShape,
});

export const WorkbenchLocationRefSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('standalone') }),
	z.strictObject({
		kind: z.literal('workspace'),
		workspaceId: identifierSchema,
	}),
]);

export const WorkbenchTerminalListInputSchema = z.strictObject({
	location: WorkbenchLocationRefSchema,
	...optionalPageShape,
});
export const WorkbenchTabListInputSchema = z.strictObject({
	location: WorkbenchLocationRefSchema,
	...optionalPageShape,
});
export const WorkbenchPanelListInputSchema = z.strictObject({
	location: WorkbenchLocationRefSchema,
	tabId: identifierSchema,
	...optionalPageShape,
});

export const WORKBENCH_TERMINAL_OUTPUT_DEFAULT_ROWS = 80;
export const WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_ROWS = 1_000;
export const WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES = 128 * 1_024;

export const WorkbenchTerminalOutputReadInputSchema = z.strictObject({
	presentationId: TerminalPresentationIdSchema,
	rows: z
		.number()
		.int()
		.min(1)
		.max(WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_ROWS)
		.default(WORKBENCH_TERMINAL_OUTPUT_DEFAULT_ROWS),
});

export const WorkbenchTerminalOutputReadOutputSchema = z
	.strictObject({
		activeScreen: z.enum(['alternate', 'primary']),
		format: z.literal('plain-text'),
		generation: terminalUint64TextSchema.refine((value) => value !== '0'),
		presentationId: TerminalPresentationIdSchema,
		requestedRows: z.number().int().min(1).max(WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_ROWS),
		returnedRows: z.number().int().min(1).max(WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_ROWS),
		revision: terminalUint64TextSchema,
		source: z.literal('tail-unwrapped'),
		startRow: terminalUint64TextSchema,
		text: z
			.string()
			.refine((value) => !containsUnpairedSurrogate(value), 'Text contains an unpaired surrogate')
			.refine(
				(value) =>
					textEncoder.encode(value).byteLength <= WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES,
				'Terminal output exceeds the public text limit',
			),
		totalRows: terminalUint64TextSchema,
		truncated: z.boolean(),
	})
	.superRefine((output, context) => {
		const startRow = BigInt(output.startRow);
		const totalRows = BigInt(output.totalRows);
		if (output.returnedRows > output.requestedRows) {
			context.addIssue({
				code: 'custom',
				message: 'Returned rows exceed the requested tail',
				path: ['returnedRows'],
			});
		}
		if (startRow + BigInt(output.returnedRows) > totalRows) {
			context.addIssue({
				code: 'custom',
				message: 'Returned rows exceed the retained Terminal range',
				path: ['startRow'],
			});
		}
		const completeTailRows = Number(
			totalRows < BigInt(output.requestedRows) ? totalRows : BigInt(output.requestedRows),
		);
		if (!output.truncated && (startRow !== 0n || output.returnedRows !== completeTailRows)) {
			context.addIssue({
				code: 'custom',
				message: 'An incomplete retained tail must be marked truncated',
				path: ['truncated'],
			});
		}
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
	placement: z.discriminatedUnion('kind', [
		z.strictObject({ kind: z.literal('local') }),
		z.strictObject({ environmentId: identifierSchema, kind: z.literal('environment') }),
	]),
	primary: z.boolean(),
	source: z.enum(['created', 'discovered', 'selected']),
});

const workspaceListItemSchema = z.strictObject({
	projectId: identifierSchema.nullable(),
	resourceScopeId: identifierSchema,
	name: z.string(),
	primaryDirectoryId: identifierSchema,
	workspaceId: identifierSchema,
});

const tabListItemSchema = z.strictObject({
	groupId: identifierSchema.nullable(),
	panelCount: z.number().int().positive(),
	pinned: z.boolean(),
	tabCustomTitle: z.string().min(1).nullable(),
	tabFallbackTitle: z.string(),
	tabId: identifierSchema,
	terminalContextId: identifierSchema,
	terminalCount: z.number().int().positive(),
});

const panelListItemSchema = z.strictObject({
	panelId: identifierSchema,
	position: z.number().int().positive(),
	tabId: identifierSchema,
	terminalContextId: identifierSchema,
	terminalCount: z.number().int().positive(),
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
export const WorkbenchTabListPageSchema = z.strictObject({
	...pageShape,
	items: z.array(tabListItemSchema).max(200),
});
export const WorkbenchPanelListPageSchema = z.strictObject({
	...pageShape,
	items: z.array(panelListItemSchema).max(200),
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
	output: { name: 'workbench.directory-list-page', schemaVersion: 2 },
	retry: 'same-request-while-retained',
	schemaVersion: 2,
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

export const WORKBENCH_TERMINAL_OUTPUT_READ_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'workbench.terminal.output.read',
	input: { name: 'workbench.terminal-output-read-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'workbench.terminal-output-read-output', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export const WORKBENCH_TAB_LIST_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'workbench.tab.list',
	input: { name: 'workbench.tab-list-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'workbench.tab-list-page', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export const WORKBENCH_PANEL_LIST_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'workbench.panel.list',
	input: { name: 'workbench.panel-list-input', schemaVersion: 1 },
	kind: 'query',
	output: { name: 'workbench.panel-list-page', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);

export type WorkbenchPageInput = z.infer<typeof WorkbenchProjectListInputSchema>;
export type WorkbenchProjectListInput = z.infer<typeof WorkbenchProjectListInputSchema>;
export type WorkbenchDirectoryListInput = z.infer<typeof WorkbenchDirectoryListInputSchema>;
export type WorkbenchWorkspaceListInput = z.infer<typeof WorkbenchWorkspaceListInputSchema>;
export type WorkbenchTabListInput = z.infer<typeof WorkbenchTabListInputSchema>;
export type WorkbenchPanelListInput = z.infer<typeof WorkbenchPanelListInputSchema>;
export type WorkbenchTerminalListInput = z.infer<typeof WorkbenchTerminalListInputSchema>;
export type WorkbenchTerminalOutputReadInput = z.infer<
	typeof WorkbenchTerminalOutputReadInputSchema
>;
export type WorkbenchProjectListPage = z.infer<typeof WorkbenchProjectListPageSchema>;
export type WorkbenchDirectoryListPage = z.infer<typeof WorkbenchDirectoryListPageSchema>;
export type WorkbenchWorkspaceListPage = z.infer<typeof WorkbenchWorkspaceListPageSchema>;
export type WorkbenchTabListPage = z.infer<typeof WorkbenchTabListPageSchema>;
export type WorkbenchPanelListPage = z.infer<typeof WorkbenchPanelListPageSchema>;
export type WorkbenchTerminalListPage = z.infer<typeof WorkbenchTerminalListPageSchema>;
export type WorkbenchTerminalOutputReadOutput = z.infer<
	typeof WorkbenchTerminalOutputReadOutputSchema
>;

function containsUnpairedSurrogate(value: string): boolean {
	for (let index = 0; index < value.length; index += 1) {
		const codeUnit = value.charCodeAt(index);
		if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
			const next = value.charCodeAt(index + 1);
			if (next < 0xdc00 || next > 0xdfff) return true;
			index += 1;
			continue;
		}
		if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) return true;
	}
	return false;
}
