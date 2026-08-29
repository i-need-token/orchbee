import {
	SemanticCommandJsonValueSchema,
	SemanticCommandOperationDescriptorSchema,
	SemanticCommandResultCodeSchema,
	type SemanticCommandTerminalResult,
} from '@orchbee/semantic-command-protocol';
import { WorkbenchProjectWorkspaceRuntime } from '@orchbee/workbench';
import {
	type WorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import {
	WORKBENCH_TERMINAL_CREATE_OPERATION,
	WorkbenchTerminalCreateInputSchema,
	WorkbenchTerminalCreateOutputSchema,
	executeWorkbenchTerminalCreate,
	type WorkbenchTerminalCreateOperationCreated,
} from '@orchbee/workbench/semantic-commands';
import type {
	TerminalPresentationId,
	TerminalPresentationOperationId,
	TerminalPresentationRemovalId,
} from '@orchbee/workbench/terminal-presentation';

import type {
	DesktopSemanticCommandHandlerSettlement,
	DesktopSemanticCommandOperation,
} from './DesktopSemanticCommandCatalog';

type CanonicalDocumentMutationDecision =
	| { readonly document: WorkbenchSharedDocument; readonly kind: 'commit' }
	| { readonly kind: 'rejected'; readonly reason: string };

type CanonicalDocumentMutationResult =
	| { readonly document: WorkbenchSharedDocument; readonly kind: 'committed' }
	| { readonly kind: 'rejected'; readonly reason: string }
	| {
			readonly kind: 'unavailable';
			readonly reason: 'invalid' | 'read-failed' | 'write-failed';
	  };

interface DesktopWorkbenchTerminalCreateOperationOptions {
	readonly createOperationId: () => TerminalPresentationOperationId;
	readonly createPresentationId: () => TerminalPresentationId;
	readonly createRemovalId: () => TerminalPresentationRemovalId;
	readonly defaultCwd: string;
	readonly initialPresentationId: TerminalPresentationId;
	readonly mutateCanonicalDocument: (
		mutation: (document: WorkbenchSharedDocument) => CanonicalDocumentMutationDecision,
		afterCommit: (document: WorkbenchSharedDocument) => void,
	) => Promise<CanonicalDocumentMutationResult>;
	readonly terminalTitle: string;
}

const ownerRejectionReasons = new Set<string>([
	'explicit-directory-not-found',
	'explicit-directory-unavailable',
	'project-not-found',
	'tab-placement-invalid',
	'terminal-creation-sealed',
	'terminal-operation-conflict',
	'terminal-presentation-conflict',
	'workspace-not-found',
]);

const cancellationReason = 'semantic-command-cancelled';

export function createDesktopWorkbenchTerminalCreateOperation(
	options: DesktopWorkbenchTerminalCreateOperationOptions,
): DesktopSemanticCommandOperation {
	const descriptor = SemanticCommandOperationDescriptorSchema.parse(
		WORKBENCH_TERMINAL_CREATE_OPERATION,
	);
	return {
		authorize: (caller) => caller.kind === 'local-user',
		canonicalizeInput: (input) => {
			const parsed = WorkbenchTerminalCreateInputSchema.safeParse(input);
			return parsed.success
				? { input: SemanticCommandJsonValueSchema.parse(parsed.data), kind: 'accepted' }
				: { kind: 'rejected' };
		},
		descriptor,
		execute: async (input, context) => {
			const parsedInput = WorkbenchTerminalCreateInputSchema.safeParse(input);
			if (!parsedInput.success) return noEffect('failed', 'operation-input-invalid');
			if (context.signal.aborted) return noEffect('cancelled', 'operation-cancelled');

			let created: WorkbenchTerminalCreateOperationCreated | undefined;
			const mutation = await options.mutateCanonicalDocument(
				(document) => {
					if (context.signal.aborted) return { kind: 'rejected', reason: cancellationReason };
					const nextRevision = workbenchSharedDocumentRevision(
						(BigInt(document.revision) + 1n).toString(),
					);
					const runtime = new WorkbenchProjectWorkspaceRuntime({
						initialDocument: document,
						standaloneCwd: options.defaultCwd,
						standaloneTerminalTitle: options.terminalTitle,
						terminalPresentation: {
							createOperationId: options.createOperationId,
							createPresentationId: options.createPresentationId,
							createRemovalId: options.createRemovalId,
							dispatchCommands: () => undefined,
							initialPresentationId: options.initialPresentationId,
							kind: 'managed',
						},
					});
					const result = executeWorkbenchTerminalCreate(runtime, {
						...(parsedInput.data.directoryId === undefined
							? {}
							: { directoryId: parsedInput.data.directoryId }),
						fallbackTitle: options.terminalTitle,
						location: parsedInput.data.location,
						placement: { groupId: null },
					});
					if (result.kind === 'rejected') return result;
					if (result.intent.kind !== 'provision') {
						return { kind: 'rejected', reason: 'terminal-intent-invalid' };
					}
					created = result;
					return { document: runtime.getSharedDocument(nextRevision), kind: 'commit' };
				},
				() => {
					context.markCommitted();
				},
			);
			if (mutation.kind === 'rejected') return rejectedMutation(mutation.reason);
			if (mutation.kind === 'unavailable') {
				return noEffect('failed', `workbench-document-${mutation.reason}`);
			}
			if (created === undefined || created.intent.kind !== 'provision') {
				return noEffect('failed', 'workbench-mutation-invalid');
			}
			const output = WorkbenchTerminalCreateOutputSchema.parse({
				intent: created.intent,
				panelId: created.panelId,
				presentationId: created.presentationId,
				revision: mutation.document.revision,
				tabId: created.tabId,
				terminalContextId: created.terminalContextId,
			});
			return {
				effect: 'committed',
				result: {
					code: SemanticCommandResultCodeSchema.parse('terminal-created'),
					kind: 'completed',
					output: SemanticCommandJsonValueSchema.parse(output),
				},
			};
		},
		isVisibleTo: (caller) => caller.kind === 'local-user',
		validateOutput: (output) => WorkbenchTerminalCreateOutputSchema.safeParse(output).success,
	};
}

function rejectedMutation(reason: string): DesktopSemanticCommandHandlerSettlement {
	if (reason === cancellationReason) return noEffect('cancelled', 'operation-cancelled');
	if (ownerRejectionReasons.has(reason)) {
		return noEffect('blocked', reason);
	}
	if (reason === 'control-unavailable' || reason === 'document-missing') {
		return noEffect('blocked', `workbench-${reason}`);
	}
	return noEffect('failed', 'workbench-mutation-invalid');
}

function noEffect(
	kind: Exclude<SemanticCommandTerminalResult['kind'], 'completed' | 'unchanged'>,
	code: string,
): DesktopSemanticCommandHandlerSettlement {
	return { effect: 'none', result: { code: SemanticCommandResultCodeSchema.parse(code), kind } };
}
