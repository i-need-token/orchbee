import {
	SemanticCommandJsonValueSchema,
	SemanticCommandOperationDescriptorSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandRequestIdentitySchema,
	type SemanticCommandJsonValue,
	type SemanticCommandOperationDescriptor,
	type SemanticCommandOperationId,
	type SemanticCommandRequestIdentity,
} from '@orchbee/semantic-command-protocol';
import {
	SemanticCommandLocalEndpointIdSchema,
	type SemanticCommandLocalEndpointId,
} from '@orchbee/semantic-command-protocol/local-http';
import {
	WORKBENCH_DIRECTORY_LIST_OPERATION,
	WORKBENCH_PROJECT_LIST_OPERATION,
	WORKBENCH_TERMINAL_CREATE_OPERATION,
	WORKBENCH_TERMINAL_LIST_OPERATION,
	WORKBENCH_WORKSPACE_LIST_OPERATION,
	WorkbenchDirectoryListInputSchema,
	WorkbenchProjectListInputSchema,
	WorkbenchTerminalCreateInputSchema,
	WorkbenchTerminalListInputSchema,
	WorkbenchWorkspaceListInputSchema,
} from '@orchbee/workbench/semantic-commands';

export type SemanticCommandCliOutput = 'json' | 'text';

interface SemanticCommandCliCommandBase {
	readonly endpointId?: SemanticCommandLocalEndpointId | undefined;
	readonly output: SemanticCommandCliOutput;
}

export type SemanticCommandCliOwnedOperation = SemanticCommandOperationDescriptor;

interface SemanticCommandCliOwnedInvocationBase extends SemanticCommandCliCommandBase {
	readonly detach: boolean;
	readonly input: SemanticCommandJsonValue;
	readonly operation: SemanticCommandCliOwnedOperation;
	readonly requestIdentity?: SemanticCommandRequestIdentity | undefined;
}

export type SemanticCommandCliCommand =
	| { readonly kind: 'skill' }
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
	  })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'project-list' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'directory-list' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'workspace-list' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'terminal-list' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'terminal-create' });

export type SemanticCommandCliArgumentParseResult =
	| { readonly code: 'cli-usage-invalid' | 'output-selection-invalid'; readonly kind: 'error' }
	| { readonly command: SemanticCommandCliCommand; readonly kind: 'parsed' };

interface ParsedOptions {
	cursor: string | undefined;
	detach: boolean;
	directoryId: string | undefined;
	endpointId: SemanticCommandLocalEndpointId | undefined;
	inputFile: string | undefined;
	limit: number | undefined;
	output: SemanticCommandCliOutput;
	projectId: string | undefined;
	requestIdentity: SemanticCommandRequestIdentity | undefined;
	standalone: boolean;
	workspaceId: string | undefined;
}

export function parseSemanticCommandCliArguments(
	arguments_: readonly string[],
): SemanticCommandCliArgumentParseResult {
	if (matches(arguments_, ['--skill'])) {
		return { command: { kind: 'skill' }, kind: 'parsed' };
	}
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
	const workbenchCommand = parseWorkbenchFacadeCommand(positionals, usedOptions, options, base);
	if (workbenchCommand !== null) return workbenchCommand;
	return usageError();
}

function parseWorkbenchFacadeCommand(
	positionals: readonly string[],
	usedOptions: ReadonlySet<string>,
	options: ParsedOptions,
	base: SemanticCommandCliCommandBase,
): Extract<SemanticCommandCliArgumentParseResult, { readonly kind: 'parsed' }> | null {
	const pageOptions = {
		...(options.cursor === undefined ? {} : { cursor: options.cursor }),
		...(options.limit === undefined ? {} : { limit: options.limit }),
	};
	if (
		matches(positionals, ['project', 'list']) &&
		onlyUses(usedOptions, ['cursor', 'limit', 'output', 'target'])
	) {
		const input = WorkbenchProjectListInputSchema.safeParse(pageOptions);
		return input.success
			? ownedInvocation(base, 'project-list', WORKBENCH_PROJECT_LIST_OPERATION, input.data)
			: null;
	}
	if (
		matches(positionals, ['directory', 'list']) &&
		onlyUses(usedOptions, ['cursor', 'limit', 'output', 'project', 'target']) &&
		options.projectId !== undefined
	) {
		const input = WorkbenchDirectoryListInputSchema.safeParse({
			...pageOptions,
			projectId: options.projectId,
		});
		return input.success
			? ownedInvocation(base, 'directory-list', WORKBENCH_DIRECTORY_LIST_OPERATION, input.data)
			: null;
	}
	if (
		matches(positionals, ['workspace', 'list']) &&
		onlyUses(usedOptions, ['cursor', 'limit', 'output', 'project', 'target']) &&
		options.projectId !== undefined
	) {
		const input = WorkbenchWorkspaceListInputSchema.safeParse({
			...pageOptions,
			projectId: options.projectId,
		});
		return input.success
			? ownedInvocation(base, 'workspace-list', WORKBENCH_WORKSPACE_LIST_OPERATION, input.data)
			: null;
	}
	const location = workbenchLocation(options);
	if (
		matches(positionals, ['terminal', 'list']) &&
		onlyUses(usedOptions, [
			'cursor',
			'limit',
			'output',
			'project',
			'standalone',
			'target',
			'workspace',
		]) &&
		location !== null
	) {
		const input = WorkbenchTerminalListInputSchema.safeParse({ ...pageOptions, location });
		return input.success
			? ownedInvocation(base, 'terminal-list', WORKBENCH_TERMINAL_LIST_OPERATION, input.data)
			: null;
	}
	if (
		matches(positionals, ['terminal', 'create']) &&
		onlyUses(usedOptions, [
			'detach',
			'directory',
			'output',
			'project',
			'request-id',
			'standalone',
			'target',
			'workspace',
		]) &&
		location !== null
	) {
		const input = WorkbenchTerminalCreateInputSchema.safeParse({
			...(options.directoryId === undefined ? {} : { directoryId: options.directoryId }),
			location,
		});
		return input.success
			? ownedInvocation(
					base,
					'terminal-create',
					WORKBENCH_TERMINAL_CREATE_OPERATION,
					input.data,
					options,
				)
			: null;
	}
	return null;
}

function ownedInvocation(
	base: SemanticCommandCliCommandBase,
	kind: Extract<SemanticCommandCliCommand, SemanticCommandCliOwnedInvocationBase>['kind'],
	operation: unknown,
	input: unknown,
	options?: ParsedOptions,
): Extract<SemanticCommandCliArgumentParseResult, { readonly kind: 'parsed' }> {
	return {
		command: {
			...base,
			detach: options?.detach ?? false,
			input: SemanticCommandJsonValueSchema.parse(input),
			kind,
			operation: SemanticCommandOperationDescriptorSchema.parse(operation),
			...(options?.requestIdentity === undefined
				? {}
				: { requestIdentity: options.requestIdentity }),
		},
		kind: 'parsed',
	};
}

function workbenchLocation(options: ParsedOptions): SemanticCommandJsonValue | null {
	if (options.standalone) {
		return options.projectId === undefined && options.workspaceId === undefined
			? { kind: 'standalone' }
			: null;
	}
	return options.projectId !== undefined && options.workspaceId !== undefined
		? { kind: 'workspace', projectId: options.projectId, workspaceId: options.workspaceId }
		: null;
}

function collectArguments(arguments_: readonly string[]):
	| {
			readonly kind: 'collected';
			readonly options: ParsedOptions;
			readonly positionals: readonly string[];
			readonly usedOptions: ReadonlySet<string>;
	  }
	| Extract<SemanticCommandCliArgumentParseResult, { readonly kind: 'error' }> {
	let cursor: string | undefined;
	let detach = false;
	let directoryId: string | undefined;
	let endpointId: SemanticCommandLocalEndpointId | undefined;
	let inputFile: string | undefined;
	let limit: number | undefined;
	let output: SemanticCommandCliOutput = 'json';
	let projectId: string | undefined;
	let requestIdentity: SemanticCommandRequestIdentity | undefined;
	let standalone = false;
	let workspaceId: string | undefined;
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
		if (option === 'detach' || option === 'standalone') {
			if (option === 'detach') detach = true;
			else standalone = true;
			continue;
		}
		if (
			option !== 'cursor' &&
			option !== 'directory' &&
			option !== 'input-file' &&
			option !== 'limit' &&
			option !== 'output' &&
			option !== 'project' &&
			option !== 'request-id' &&
			option !== 'target' &&
			option !== 'workspace'
		) {
			return usageError();
		}
		const value = arguments_[++index];
		if (value === undefined || value.length === 0 || value.startsWith('--')) return usageError();
		if (option === 'cursor') {
			if (value.length > 4096) return usageError();
			cursor = value;
		} else if (option === 'directory') {
			if (value.length > 512) return usageError();
			directoryId = value;
		} else if (option === 'input-file') {
			if (value.length > 4096) return usageError();
			inputFile = value;
		} else if (option === 'limit') {
			if (!/^[1-9]\d{0,2}$/u.test(value)) return usageError();
			limit = Number(value);
			if (limit > 200) return usageError();
		} else if (option === 'output') {
			if (value !== 'json' && value !== 'text') {
				return { code: 'output-selection-invalid', kind: 'error' };
			}
			output = value;
		} else if (option === 'project') {
			if (value.length > 512) return usageError();
			projectId = value;
		} else if (option === 'request-id') {
			const parsed = SemanticCommandRequestIdentitySchema.safeParse(value);
			if (!parsed.success) return usageError();
			requestIdentity = parsed.data;
		} else if (option === 'target') {
			const parsed = SemanticCommandLocalEndpointIdSchema.safeParse(value);
			if (!parsed.success) return usageError();
			endpointId = parsed.data;
		} else {
			if (value.length > 512) return usageError();
			workspaceId = value;
		}
	}
	return {
		kind: 'collected',
		options: {
			cursor,
			detach,
			directoryId,
			endpointId,
			inputFile,
			limit,
			output,
			projectId,
			requestIdentity,
			standalone,
			workspaceId,
		},
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
