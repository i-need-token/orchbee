import {
	SemanticCommandOperationIdSchema,
	SemanticCommandRequestIdentitySchema,
	type SemanticCommandOperationId,
	type SemanticCommandRequestIdentity,
} from '@orchbee/semantic-command-protocol';
import {
	SemanticCommandLocalEndpointIdSchema,
	type SemanticCommandLocalEndpointId,
} from '@orchbee/semantic-command-protocol/local-http';

export type SemanticCommandCliOutput = 'json' | 'text';

interface SemanticCommandCliCommandBase {
	readonly endpointId?: SemanticCommandLocalEndpointId | undefined;
	readonly output: SemanticCommandCliOutput;
}

export type SemanticCommandCliCommand =
	| (SemanticCommandCliCommandBase & { readonly kind: 'status' })
	| (SemanticCommandCliCommandBase & { readonly kind: 'operation-list' })
	| (SemanticCommandCliCommandBase & {
			readonly kind: 'operation-describe';
			readonly operationId: SemanticCommandOperationId;
	  })
	| (SemanticCommandCliCommandBase & {
			readonly detach: boolean;
			readonly inputFile: string;
			readonly kind: 'operation-invoke';
			readonly operationId: SemanticCommandOperationId;
			readonly requestIdentity?: SemanticCommandRequestIdentity | undefined;
	  })
	| (SemanticCommandCliCommandBase & {
			readonly kind: 'execution-cancel';
			readonly referenceIdentity: string;
	  })
	| (SemanticCommandCliCommandBase & {
			readonly kind: 'execution-get';
			readonly referenceIdentity: string;
	  })
	| (SemanticCommandCliCommandBase & {
			readonly kind: 'execution-wait';
			readonly referenceIdentity: string;
	  });

export type SemanticCommandCliArgumentParseResult =
	| { readonly code: 'cli-usage-invalid' | 'output-selection-invalid'; readonly kind: 'error' }
	| { readonly command: SemanticCommandCliCommand; readonly kind: 'parsed' };

interface ParsedOptions {
	detach: boolean;
	endpointId: SemanticCommandLocalEndpointId | undefined;
	inputFile: string | undefined;
	output: SemanticCommandCliOutput;
	requestIdentity: SemanticCommandRequestIdentity | undefined;
}

export function parseSemanticCommandCliArguments(
	arguments_: readonly string[],
): SemanticCommandCliArgumentParseResult {
	const parsed = collectArguments(arguments_);
	if (parsed.kind === 'error') return parsed;
	const { options, positionals, usedOptions } = parsed;
	const base = {
		...(options.endpointId === undefined ? {} : { endpointId: options.endpointId }),
		output: options.output,
	};
	if (matches(positionals, ['status']) && onlyUses(usedOptions, ['output', 'target'])) {
		return { command: { ...base, kind: 'status' }, kind: 'parsed' };
	}
	if (matches(positionals, ['operation', 'list']) && onlyUses(usedOptions, ['output', 'target'])) {
		return { command: { ...base, kind: 'operation-list' }, kind: 'parsed' };
	}
	if (
		positionals.length === 3 &&
		positionals[0] === 'operation' &&
		positionals[1] === 'describe' &&
		onlyUses(usedOptions, ['output', 'target'])
	) {
		const operationId = SemanticCommandOperationIdSchema.safeParse(positionals[2]);
		return operationId.success
			? {
					command: { ...base, kind: 'operation-describe', operationId: operationId.data },
					kind: 'parsed',
				}
			: usageError();
	}
	if (
		positionals.length === 3 &&
		positionals[0] === 'operation' &&
		positionals[1] === 'invoke' &&
		onlyUses(usedOptions, ['detach', 'input-file', 'output', 'request-id', 'target']) &&
		options.inputFile !== undefined
	) {
		const operationId = SemanticCommandOperationIdSchema.safeParse(positionals[2]);
		if (!operationId.success) return usageError();
		return {
			command: {
				...base,
				detach: options.detach,
				inputFile: options.inputFile,
				kind: 'operation-invoke',
				operationId: operationId.data,
				...(options.requestIdentity === undefined
					? {}
					: { requestIdentity: options.requestIdentity }),
			},
			kind: 'parsed',
		};
	}
	if (
		positionals.length === 3 &&
		positionals[0] === 'execution' &&
		(positionals[1] === 'get' || positionals[1] === 'wait' || positionals[1] === 'cancel') &&
		onlyUses(usedOptions, ['output', 'target'])
	) {
		const referenceIdentity = positionals[2];
		if (!isExecutionReferenceIdentity(referenceIdentity)) return usageError();
		return {
			command: {
				...base,
				kind: `execution-${positionals[1]}`,
				referenceIdentity,
			},
			kind: 'parsed',
		};
	}
	return usageError();
}

function collectArguments(arguments_: readonly string[]):
	| {
			readonly kind: 'collected';
			readonly options: ParsedOptions;
			readonly positionals: readonly string[];
			readonly usedOptions: ReadonlySet<string>;
	  }
	| Extract<SemanticCommandCliArgumentParseResult, { readonly kind: 'error' }> {
	let detach = false;
	let endpointId: SemanticCommandLocalEndpointId | undefined;
	let inputFile: string | undefined;
	let output: SemanticCommandCliOutput = 'json';
	let requestIdentity: SemanticCommandRequestIdentity | undefined;
	const positionals: string[] = [];
	const usedOptions = new Set<string>();
	for (let index = 0; index < arguments_.length; index += 1) {
		const argument = arguments_[index];
		if (argument === undefined) return usageError();
		if (!argument.startsWith('--')) {
			positionals.push(argument);
			continue;
		}
		const option = argument.slice(2);
		if (usedOptions.has(option)) return usageError();
		usedOptions.add(option);
		if (option === 'detach') {
			detach = true;
			continue;
		}
		if (
			option !== 'input-file' &&
			option !== 'output' &&
			option !== 'request-id' &&
			option !== 'target'
		) {
			return usageError();
		}
		const value = arguments_[++index];
		if (value === undefined || value.length === 0 || value.startsWith('--')) return usageError();
		if (option === 'input-file') {
			if (value.length > 4096) return usageError();
			inputFile = value;
		} else if (option === 'output') {
			if (value !== 'json' && value !== 'text') {
				return { code: 'output-selection-invalid', kind: 'error' };
			}
			output = value;
		} else if (option === 'request-id') {
			const parsed = SemanticCommandRequestIdentitySchema.safeParse(value);
			if (!parsed.success) return usageError();
			requestIdentity = parsed.data;
		} else {
			const parsed = SemanticCommandLocalEndpointIdSchema.safeParse(value);
			if (!parsed.success) return usageError();
			endpointId = parsed.data;
		}
	}
	return {
		kind: 'collected',
		options: { detach, endpointId, inputFile, output, requestIdentity },
		positionals,
		usedOptions,
	};
}

function matches(actual: readonly string[], expected: readonly string[]): boolean {
	return (
		actual.length === expected.length && actual.every((value, index) => value === expected[index])
	);
}

function onlyUses(actual: ReadonlySet<string>, allowed: readonly string[]): boolean {
	return [...actual].every((option) => allowed.includes(option));
}

function isExecutionReferenceIdentity(value: string | undefined): value is string {
	return value !== undefined && SemanticCommandRequestIdentitySchema.safeParse(value).success;
}

function usageError(): Extract<SemanticCommandCliArgumentParseResult, { readonly kind: 'error' }> {
	return { code: 'cli-usage-invalid', kind: 'error' };
}
