import {
	SemanticCommandJsonValueSchema,
	type SemanticCommandJsonValue,
} from '@orchbee/semantic-command-protocol';

import type { SemanticCommandCliOutput } from './parseSemanticCommandCliArguments';

export const SEMANTIC_COMMAND_CLI_OUTPUT_VERSION = 1 as const;

export interface SemanticCommandCliOutputDocument {
	readonly cliVersion: typeof SEMANTIC_COMMAND_CLI_OUTPUT_VERSION;
	readonly command: string | null;
	readonly code?: string | undefined;
	readonly kind: 'error' | 'result';
	readonly requestIdentity?: string | undefined;
	readonly result?: SemanticCommandJsonValue | undefined;
}

export interface SemanticCommandCliRunResult {
	readonly exitCode: 0 | 2 | 3 | 4 | 5 | 6;
	readonly stderr: string;
	readonly stdout: string;
}

export function cliErrorResult(
	command: string | null,
	code: string,
	output: SemanticCommandCliOutput = 'json',
	requestIdentity?: string | undefined,
): SemanticCommandCliRunResult {
	return serializeCliResult(
		{
			cliVersion: SEMANTIC_COMMAND_CLI_OUTPUT_VERSION,
			command,
			code,
			kind: 'error',
			...(requestIdentity === undefined ? {} : { requestIdentity }),
		},
		2,
		output,
	);
}

export function cliProtocolResult(
	command: string,
	result: unknown,
	exitCode: SemanticCommandCliRunResult['exitCode'],
	output: SemanticCommandCliOutput,
	requestIdentity?: string | undefined,
): SemanticCommandCliRunResult {
	return serializeCliResult(
		{
			cliVersion: SEMANTIC_COMMAND_CLI_OUTPUT_VERSION,
			command,
			kind: 'result',
			...(requestIdentity === undefined ? {} : { requestIdentity }),
			result: SemanticCommandJsonValueSchema.parse(result),
		},
		exitCode,
		output,
	);
}

export function cliFailureResult(
	command: string,
	code: string,
	exitCode: SemanticCommandCliRunResult['exitCode'],
	output: SemanticCommandCliOutput,
	requestIdentity?: string | undefined,
): SemanticCommandCliRunResult {
	return serializeCliResult(
		{
			cliVersion: SEMANTIC_COMMAND_CLI_OUTPUT_VERSION,
			command,
			code,
			kind: 'error',
			...(requestIdentity === undefined ? {} : { requestIdentity }),
		},
		exitCode,
		output,
	);
}

function serializeCliResult(
	document: SemanticCommandCliOutputDocument,
	exitCode: SemanticCommandCliRunResult['exitCode'],
	output: SemanticCommandCliOutput,
): SemanticCommandCliRunResult {
	return {
		exitCode,
		stderr: '',
		stdout: `${JSON.stringify(document, null, output === 'text' ? 2 : undefined)}\n`,
	};
}
