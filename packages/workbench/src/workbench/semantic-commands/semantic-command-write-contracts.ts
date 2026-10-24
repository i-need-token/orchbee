import { z } from 'zod';

import {
	TerminalPresentationIdSchema,
	TerminalPresentationOperationIdSchema,
} from '../../terminal-presentation';
import { WorkbenchSharedDocumentRevisionSchema } from '../document';
import { WorkbenchLocationRefSchema } from './semantic-command-read-contracts';

const identifierSchema = z.string().min(1).max(512);

export const WorkbenchTerminalCreateInputSchema = z.strictObject({
	directoryId: identifierSchema.optional(),
	location: WorkbenchLocationRefSchema,
});

export const WorkbenchTerminalCreateOutputSchema = z.strictObject({
	intent: z.strictObject({
		kind: z.literal('provision'),
		operationId: TerminalPresentationOperationIdSchema,
	}),
	panelId: identifierSchema,
	presentationId: TerminalPresentationIdSchema,
	revision: WorkbenchSharedDocumentRevisionSchema,
	tabId: identifierSchema,
	terminalContextId: identifierSchema,
});

export const WORKBENCH_TERMINAL_CREATE_OPERATION = Object.freeze({
	confirmation: 'noninteractive-allowed',
	id: 'workbench.terminal.create',
	input: { name: 'workbench.terminal-create-input', schemaVersion: 1 },
	kind: 'command',
	output: { name: 'workbench.terminal-create-output', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
} as const);
