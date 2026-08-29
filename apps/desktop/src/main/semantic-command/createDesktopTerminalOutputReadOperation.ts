import {
	SemanticCommandJsonValueSchema,
	SemanticCommandOperationDescriptorSchema,
	SemanticCommandResultCodeSchema,
	type SemanticCommandTerminalResult,
} from '@orchbee/semantic-command-protocol';
import {
	WORKBENCH_TERMINAL_OUTPUT_READ_OPERATION,
	WorkbenchTerminalOutputReadInputSchema,
	WorkbenchTerminalOutputReadOutputSchema,
} from '@orchbee/workbench/semantic-commands';

import type { DesktopTerminalOutputReader } from '../terminal-output-read';
import type {
	DesktopSemanticCommandHandlerSettlement,
	DesktopSemanticCommandOperation,
} from './DesktopSemanticCommandCatalog';

interface DesktopTerminalOutputReadOperationOptions {
	readonly reader: Pick<DesktopTerminalOutputReader, 'read'>;
}

export function createDesktopTerminalOutputReadOperation(
	options: DesktopTerminalOutputReadOperationOptions,
): DesktopSemanticCommandOperation {
	const descriptor = SemanticCommandOperationDescriptorSchema.parse(
		WORKBENCH_TERMINAL_OUTPUT_READ_OPERATION,
	);
	return {
		authorize: (caller) => caller.kind === 'local-user',
		canonicalizeInput: (input) => {
			const parsed = WorkbenchTerminalOutputReadInputSchema.safeParse(input);
			return parsed.success
				? { input: SemanticCommandJsonValueSchema.parse(parsed.data), kind: 'accepted' }
				: { kind: 'rejected' };
		},
		descriptor,
		execute: async (input, context) => {
			const parsed = WorkbenchTerminalOutputReadInputSchema.safeParse(input);
			if (!parsed.success) return noEffect('failed', 'operation-input-invalid');
			if (context.signal.aborted) {
				return noEffect('cancelled', 'terminal-output-read-cancelled');
			}
			const result = await options.reader.read(
				parsed.data.presentationId,
				parsed.data.rows,
				context.signal,
			);
			switch (result.kind) {
				case 'completed':
					return {
						effect: 'none',
						result: {
							code: SemanticCommandResultCodeSchema.parse('terminal-output-read'),
							kind: 'completed',
							output: SemanticCommandJsonValueSchema.parse(result.output),
						},
					};
				case 'cancelled':
					return noEffect('cancelled', 'terminal-output-read-cancelled');
				case 'cleanup-failed':
					return noEffect('failed', 'terminal-output-read-cleanup-failed');
				case 'stale':
					return noEffect('blocked', 'terminal-output-read-stale');
				case 'target-retired':
					return noEffect('blocked', 'terminal-output-target-retired');
				case 'unavailable':
					return noEffect('failed', 'terminal-output-read-unavailable');
			}
		},
		isVisibleTo: (caller) => caller.kind === 'local-user',
		validateOutput: (output) => WorkbenchTerminalOutputReadOutputSchema.safeParse(output).success,
	};
}

function noEffect(
	kind: Exclude<SemanticCommandTerminalResult['kind'], 'completed' | 'unchanged'>,
	code: string,
): DesktopSemanticCommandHandlerSettlement {
	return { effect: 'none', result: { code: SemanticCommandResultCodeSchema.parse(code), kind } };
}
