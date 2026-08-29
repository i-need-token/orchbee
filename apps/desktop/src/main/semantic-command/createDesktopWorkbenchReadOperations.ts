import {
	SemanticCommandJsonValueSchema,
	SemanticCommandOperationDescriptorSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandResultCodeSchema,
	type SemanticCommandCoordinatorGeneration,
	type SemanticCommandOperationId,
	type SemanticCommandResultCode,
	type SemanticCommandTerminalResult,
} from '@orchbee/semantic-command-protocol';
import type { WorkbenchSharedDocument } from '@orchbee/workbench/document';
import {
	WORKBENCH_DIRECTORY_LIST_OPERATION,
	WORKBENCH_PANEL_LIST_OPERATION,
	WORKBENCH_PROJECT_LIST_OPERATION,
	WORKBENCH_TAB_LIST_OPERATION,
	WORKBENCH_TERMINAL_LIST_OPERATION,
	WORKBENCH_WORKSPACE_LIST_OPERATION,
	WorkbenchDirectoryListInputSchema,
	WorkbenchDirectoryListPageSchema,
	WorkbenchPanelListInputSchema,
	WorkbenchPanelListPageSchema,
	WorkbenchProjectListInputSchema,
	WorkbenchProjectListPageSchema,
	WorkbenchTabListInputSchema,
	WorkbenchTabListPageSchema,
	WorkbenchTerminalListInputSchema,
	WorkbenchTerminalListPageSchema,
	WorkbenchWorkspaceListInputSchema,
	WorkbenchWorkspaceListPageSchema,
	projectWorkbenchDirectoryList,
	projectWorkbenchPanelList,
	projectWorkbenchProjectList,
	projectWorkbenchTabList,
	projectWorkbenchTerminalList,
	projectWorkbenchWorkspaceList,
	type WorkbenchLocationRef,
	type WorkbenchPageInput,
} from '@orchbee/workbench/semantic-commands';
import type { z } from 'zod';

import type {
	DesktopSemanticCommandHandlerSettlement,
	DesktopSemanticCommandOperation,
} from './DesktopSemanticCommandCatalog';
import { DesktopSemanticCommandPageCursor } from './DesktopSemanticCommandPageCursor';

interface DesktopWorkbenchReadOperationOptions {
	readonly coordinatorGeneration: SemanticCommandCoordinatorGeneration;
	readonly cursorSecret: Uint8Array;
	readonly getDocument: () => WorkbenchSharedDocument | null;
}

interface WorkbenchReadProjection<Item> {
	readonly items: readonly Item[];
	readonly revision: WorkbenchSharedDocument['revision'];
}

type WorkbenchReadProjectionResult<Item> =
	| WorkbenchReadProjection<Item>
	| {
			readonly kind: 'rejected';
			readonly reason: 'project-not-found' | 'tab-not-found' | 'workspace-not-found';
	  };

interface ReadOperationDescriptor {
	readonly confirmation: 'noninteractive-allowed';
	readonly id: string;
	readonly input: { readonly name: string; readonly schemaVersion: number };
	readonly kind: 'query';
	readonly output: { readonly name: string; readonly schemaVersion: number };
	readonly retry: 'same-request-while-retained';
	readonly schemaVersion: number;
}

interface ReadOperationDefinition<Input extends WorkbenchPageInput, Item> {
	readonly descriptor: ReadOperationDescriptor;
	readonly inputSchema: z.ZodType<Input>;
	readonly outputSchema: z.ZodType<unknown>;
	readonly project: (
		document: WorkbenchSharedDocument,
		input: Input,
	) => WorkbenchReadProjectionResult<Item>;
	readonly resultCode: SemanticCommandResultCode;
	readonly target: (input: Input) => string;
}

export function createDesktopWorkbenchReadOperations(
	options: DesktopWorkbenchReadOperationOptions,
): readonly DesktopSemanticCommandOperation[] {
	const cursor = new DesktopSemanticCommandPageCursor(options.cursorSecret);
	return [
		createReadOperation(options, cursor, {
			descriptor: WORKBENCH_PROJECT_LIST_OPERATION,
			inputSchema: WorkbenchProjectListInputSchema,
			outputSchema: WorkbenchProjectListPageSchema,
			project: (document) => projectWorkbenchProjectList(document),
			resultCode: SemanticCommandResultCodeSchema.parse('projects-listed'),
			target: () => 'projects',
		}),
		createReadOperation(options, cursor, {
			descriptor: WORKBENCH_DIRECTORY_LIST_OPERATION,
			inputSchema: WorkbenchDirectoryListInputSchema,
			outputSchema: WorkbenchDirectoryListPageSchema,
			project: (document, input) => projectWorkbenchDirectoryList(document, input.projectId),
			resultCode: SemanticCommandResultCodeSchema.parse('directories-listed'),
			target: (input) => projectCursorTarget(input.projectId),
		}),
		createReadOperation(options, cursor, {
			descriptor: WORKBENCH_WORKSPACE_LIST_OPERATION,
			inputSchema: WorkbenchWorkspaceListInputSchema,
			outputSchema: WorkbenchWorkspaceListPageSchema,
			project: (document, input) => projectWorkbenchWorkspaceList(document, input.projectId),
			resultCode: SemanticCommandResultCodeSchema.parse('workspaces-listed'),
			target: (input) => projectCursorTarget(input.projectId),
		}),
		createReadOperation(options, cursor, {
			descriptor: WORKBENCH_TAB_LIST_OPERATION,
			inputSchema: WorkbenchTabListInputSchema,
			outputSchema: WorkbenchTabListPageSchema,
			project: (document, input) => projectWorkbenchTabList(document, input.location),
			resultCode: SemanticCommandResultCodeSchema.parse('tabs-listed'),
			target: (input) => locationCursorTarget(input.location),
		}),
		createReadOperation(options, cursor, {
			descriptor: WORKBENCH_PANEL_LIST_OPERATION,
			inputSchema: WorkbenchPanelListInputSchema,
			outputSchema: WorkbenchPanelListPageSchema,
			project: (document, input) =>
				projectWorkbenchPanelList(document, input.location, input.tabId),
			resultCode: SemanticCommandResultCodeSchema.parse('panels-listed'),
			target: (input) => panelCursorTarget(input.location, input.tabId),
		}),
		createReadOperation(options, cursor, {
			descriptor: WORKBENCH_TERMINAL_LIST_OPERATION,
			inputSchema: WorkbenchTerminalListInputSchema,
			outputSchema: WorkbenchTerminalListPageSchema,
			project: (document, input) => projectWorkbenchTerminalList(document, input.location),
			resultCode: SemanticCommandResultCodeSchema.parse('terminals-listed'),
			target: (input) => locationCursorTarget(input.location),
		}),
	];
}

function projectCursorTarget(projectId: string): string {
	return JSON.stringify(['project', projectId]);
}

function locationCursorTarget(location: WorkbenchLocationRef): string {
	return location.kind === 'standalone'
		? JSON.stringify(['standalone'])
		: JSON.stringify(['workspace', location.projectId, location.workspaceId]);
}

function panelCursorTarget(location: WorkbenchLocationRef, tabId: string): string {
	return location.kind === 'standalone'
		? JSON.stringify(['standalone', 'tab', tabId])
		: JSON.stringify(['workspace', location.projectId, location.workspaceId, 'tab', tabId]);
}

function createReadOperation<Input extends WorkbenchPageInput, Item>(
	options: DesktopWorkbenchReadOperationOptions,
	cursor: DesktopSemanticCommandPageCursor,
	definition: ReadOperationDefinition<Input, Item>,
): DesktopSemanticCommandOperation {
	const descriptor = SemanticCommandOperationDescriptorSchema.parse(definition.descriptor);
	const operationId = SemanticCommandOperationIdSchema.parse(descriptor.id);
	return {
		authorize: (caller) => caller.kind === 'local-user',
		canonicalizeInput: (input) => {
			const parsed = definition.inputSchema.safeParse(input);
			return parsed.success
				? { kind: 'accepted', input: SemanticCommandJsonValueSchema.parse(parsed.data) }
				: { kind: 'rejected' };
		},
		descriptor,
		execute: async (input, context) => {
			if (context.signal.aborted) return noEffect('cancelled', 'query-cancelled');
			const parsedInput = definition.inputSchema.safeParse(input);
			if (!parsedInput.success) return noEffect('failed', 'operation-input-invalid');
			const document = options.getDocument();
			if (document === null) return noEffect('blocked', 'workbench-document-missing');
			const projected = definition.project(document, parsedInput.data);
			if (isRejectedProjection(projected)) {
				return noEffect('blocked', projected.reason);
			}
			return pageProjection(
				options.coordinatorGeneration,
				cursor,
				operationId,
				definition,
				parsedInput.data,
				projected,
			);
		},
		isVisibleTo: (caller) => caller.kind === 'local-user',
		validateOutput: (output) => definition.outputSchema.safeParse(output).success,
	};
}

function pageProjection<Input extends WorkbenchPageInput, Item>(
	coordinatorGeneration: SemanticCommandCoordinatorGeneration,
	cursor: DesktopSemanticCommandPageCursor,
	operationId: SemanticCommandOperationId,
	definition: ReadOperationDefinition<Input, Item>,
	input: Input,
	projection: WorkbenchReadProjection<Item>,
): DesktopSemanticCommandHandlerSettlement {
	const cursorContext = {
		coordinatorGeneration,
		operationId,
		revision: projection.revision,
		target: definition.target(input),
	};
	const resolved = cursor.resolve(input.cursor, cursorContext);
	if (resolved.kind === 'invalid') return noEffect('blocked', 'page-cursor-invalid');
	if (resolved.kind === 'stale') return noEffect('blocked', 'page-cursor-stale');
	if (resolved.offset > projection.items.length) {
		return noEffect('blocked', 'page-cursor-invalid');
	}
	const end = Math.min(projection.items.length, resolved.offset + input.limit);
	const output = SemanticCommandJsonValueSchema.parse({
		items: projection.items.slice(resolved.offset, end),
		nextCursor: end < projection.items.length ? cursor.create(cursorContext, end) : null,
		revision: projection.revision,
	});
	return {
		effect: 'none',
		result: { kind: 'completed', code: definition.resultCode, output },
	};
}

function noEffect(
	kind: Exclude<SemanticCommandTerminalResult['kind'], 'completed' | 'unchanged'>,
	code: string,
): DesktopSemanticCommandHandlerSettlement {
	return { effect: 'none', result: { kind, code: SemanticCommandResultCodeSchema.parse(code) } };
}

function isRejectedProjection<Item>(
	projection: WorkbenchReadProjectionResult<Item>,
): projection is Extract<WorkbenchReadProjectionResult<Item>, { readonly kind: 'rejected' }> {
	return 'kind' in projection && projection.kind === 'rejected';
}
