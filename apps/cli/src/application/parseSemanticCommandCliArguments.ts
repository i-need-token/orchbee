import {
	AGENT_SESSION_CHANGES_OPERATION,
	AGENT_SESSION_EXPLAIN_OPERATION,
	AGENT_SESSION_GET_OPERATION,
	AGENT_SESSION_LIST_OPERATION,
	AGENT_SESSION_WAIT_OPERATION,
	AgentSessionChangesInputSchema,
	AgentSessionListInputSchema,
	AgentSessionTargetInputSchema,
	AgentSessionWaitInputSchema,
	AgentSessionIdSchema,
	AgentWaitConditionSchema,
	type AgentWaitCondition,
} from '@orchbee/agent';
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
	WORKBENCH_PANEL_LIST_OPERATION,
	WORKBENCH_PROJECT_LIST_OPERATION,
	WORKBENCH_TAB_LIST_OPERATION,
	WORKBENCH_TERMINAL_CREATE_OPERATION,
	WORKBENCH_TERMINAL_LIST_OPERATION,
	WORKBENCH_TERMINAL_OUTPUT_READ_OPERATION,
	WORKBENCH_WORKSPACE_LIST_OPERATION,
	WorkbenchDirectoryListInputSchema,
	WorkbenchPanelListInputSchema,
	WorkbenchProjectListInputSchema,
	WorkbenchTabListInputSchema,
	WorkbenchTerminalCreateInputSchema,
	WorkbenchTerminalListInputSchema,
	WorkbenchTerminalOutputReadInputSchema,
	WorkbenchWorkspaceListInputSchema,
} from '@orchbee/workbench/semantic-commands';

import {
	resolveSemanticCommandCliDiscovery,
	type SemanticCommandCliHelpTopic,
	type SemanticCommandCliShell,
} from './semantic-command-cli-grammar';

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
	| { readonly kind: 'help'; readonly topic: SemanticCommandCliHelpTopic }
	| { readonly kind: 'version' }
	| { readonly kind: 'completion'; readonly shell: SemanticCommandCliShell }
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
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'tab-list' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'panel-list' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'terminal-list' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'terminal-read' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'terminal-create' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'agent-list' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'agent-get' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'agent-explain' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'agent-changes' })
	| (SemanticCommandCliOwnedInvocationBase & { readonly kind: 'agent-wait' });

export type SemanticCommandCliArgumentParseResult =
	| { readonly code: 'cli-usage-invalid' | 'output-selection-invalid'; readonly kind: 'error' }
	| { readonly command: SemanticCommandCliCommand; readonly kind: 'parsed' };

interface ParsedOptions {
	afterRevision: number | undefined;
	all: boolean;
	cursor: string | undefined;
	detach: boolean;
	directoryId: string | undefined;
	endpointId: SemanticCommandLocalEndpointId | undefined;
	inputFile: string | undefined;
	limit: number | undefined;
	output: SemanticCommandCliOutput;
	projectId: string | undefined;
	requestIdentity: SemanticCommandRequestIdentity | undefined;
	rows: number | undefined;
	standalone: boolean;
	tabId: string | undefined;
	timeoutMilliseconds: number | undefined;
	until: readonly AgentWaitCondition[];
	workspaceId: string | undefined;
}

export function parseSemanticCommandCliArguments(
	arguments_: readonly string[],
): SemanticCommandCliArgumentParseResult {
	if (matches(arguments_, ['--skill'])) {
		return { command: { kind: 'skill' }, kind: 'parsed' };
	}
	const discovery = resolveSemanticCommandCliDiscovery(arguments_);
	if (discovery !== null) {
		return { command: discovery, kind: 'parsed' };
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
	const agentCommand = parseAgentFacadeCommand(positionals, usedOptions, options, base);
	if (agentCommand !== null) return agentCommand;
	const workbenchCommand = parseWorkbenchFacadeCommand(positionals, usedOptions, options, base);
	if (workbenchCommand !== null) return workbenchCommand;
	return usageError();
}

function parseAgentFacadeCommand(
	positionals: readonly string[],
	usedOptions: ReadonlySet<string>,
	options: ParsedOptions,
	base: SemanticCommandCliCommandBase,
): Extract<SemanticCommandCliArgumentParseResult, { readonly kind: 'parsed' }> | null {
	if (
		matches(positionals, ['agent', 'list']) &&
		onlyUses(usedOptions, ['all', 'cursor', 'limit', 'output', 'target'])
	) {
		const input = AgentSessionListInputSchema.safeParse({
			...(options.cursor === undefined ? {} : { cursor: options.cursor }),
			includeEnded: options.all,
			...(options.limit === undefined ? {} : { limit: options.limit }),
		});
		return input.success
			? ownedInvocation(base, 'agent-list', AGENT_SESSION_LIST_OPERATION, input.data)
			: null;
	}
	if (
		matches(positionals, ['agent', 'changes']) &&
		onlyUses(usedOptions, [
			'cursor',
			'detach',
			'limit',
			'output',
			'request-id',
			'target',
			'timeout',
		])
	) {
		const input = AgentSessionChangesInputSchema.safeParse({
			...(options.cursor === undefined ? {} : { cursor: options.cursor }),
			...(options.limit === undefined ? {} : { limit: options.limit }),
			...(options.timeoutMilliseconds === undefined
				? {}
				: { timeoutMilliseconds: options.timeoutMilliseconds }),
		});
		return input.success
			? ownedInvocation(base, 'agent-changes', AGENT_SESSION_CHANGES_OPERATION, input.data, options)
			: null;
	}
	if (
		positionals.length === 3 &&
		positionals[0] === 'agent' &&
		(positionals[1] === 'get' || positionals[1] === 'explain') &&
		onlyUses(usedOptions, ['output', 'target'])
	) {
		const sessionId = AgentSessionIdSchema.safeParse(positionals[2]);
		if (!sessionId.success) return null;
		const input = AgentSessionTargetInputSchema.parse({ sessionId: sessionId.data });
		return positionals[1] === 'get'
			? ownedInvocation(base, 'agent-get', AGENT_SESSION_GET_OPERATION, input)
			: ownedInvocation(base, 'agent-explain', AGENT_SESSION_EXPLAIN_OPERATION, input);
	}
	if (
		positionals.length === 3 &&
		positionals[0] === 'agent' &&
		positionals[1] === 'wait' &&
		onlyUses(usedOptions, [
			'after-revision',
			'detach',
			'output',
			'request-id',
			'target',
			'timeout',
			'until',
		])
	) {
		const sessionId = AgentSessionIdSchema.safeParse(positionals[2]);
		if (!sessionId.success) return null;
		const input = AgentSessionWaitInputSchema.safeParse({
			...(options.afterRevision === undefined ? {} : { afterRevision: options.afterRevision }),
			sessionId: sessionId.data,
			...(options.timeoutMilliseconds === undefined
				? {}
				: { timeoutMilliseconds: options.timeoutMilliseconds }),
			...(options.until.length === 0 ? {} : { until: options.until }),
		});
		return input.success
			? ownedInvocation(base, 'agent-wait', AGENT_SESSION_WAIT_OPERATION, input.data, options)
			: null;
	}
	return null;
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
		matches(positionals, ['tab', 'list']) &&
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
		const input = WorkbenchTabListInputSchema.safeParse({ ...pageOptions, location });
		return input.success
			? ownedInvocation(base, 'tab-list', WORKBENCH_TAB_LIST_OPERATION, input.data)
			: null;
	}
	if (
		matches(positionals, ['panel', 'list']) &&
		onlyUses(usedOptions, [
			'cursor',
			'limit',
			'output',
			'project',
			'standalone',
			'tab',
			'target',
			'workspace',
		]) &&
		location !== null &&
		options.tabId !== undefined
	) {
		const input = WorkbenchPanelListInputSchema.safeParse({
			...pageOptions,
			location,
			tabId: options.tabId,
		});
		return input.success
			? ownedInvocation(base, 'panel-list', WORKBENCH_PANEL_LIST_OPERATION, input.data)
			: null;
	}
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
		positionals.length === 3 &&
		positionals[0] === 'terminal' &&
		positionals[1] === 'read' &&
		onlyUses(usedOptions, ['detach', 'output', 'request-id', 'rows', 'target'])
	) {
		const input = WorkbenchTerminalOutputReadInputSchema.safeParse({
			presentationId: positionals[2],
			...(options.rows === undefined ? {} : { rows: options.rows }),
		});
		return input.success
			? ownedInvocation(
					base,
					'terminal-read',
					WORKBENCH_TERMINAL_OUTPUT_READ_OPERATION,
					input.data,
					options,
				)
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
	let afterRevision: number | undefined;
	let all = false;
	let cursor: string | undefined;
	let detach = false;
	let directoryId: string | undefined;
	let endpointId: SemanticCommandLocalEndpointId | undefined;
	let inputFile: string | undefined;
	let limit: number | undefined;
	let output: SemanticCommandCliOutput = 'json';
	let projectId: string | undefined;
	let requestIdentity: SemanticCommandRequestIdentity | undefined;
	let rows: number | undefined;
	let standalone = false;
	let tabId: string | undefined;
	let timeoutMilliseconds: number | undefined;
	const until: AgentWaitCondition[] = [];
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
		if (usedOptions.has(option) && option !== 'until') return usageError();
		usedOptions.add(option);
		if (option === 'all' || option === 'detach' || option === 'standalone') {
			if (option === 'detach') detach = true;
			else if (option === 'standalone') standalone = true;
			else all = true;
			continue;
		}
		if (
			option !== 'after-revision' &&
			option !== 'cursor' &&
			option !== 'directory' &&
			option !== 'input-file' &&
			option !== 'limit' &&
			option !== 'output' &&
			option !== 'project' &&
			option !== 'request-id' &&
			option !== 'rows' &&
			option !== 'tab' &&
			option !== 'target' &&
			option !== 'timeout' &&
			option !== 'until' &&
			option !== 'workspace'
		) {
			return usageError();
		}
		const value = arguments_[++index];
		if (value === undefined || value.length === 0 || value.startsWith('--')) return usageError();
		if (option === 'after-revision') {
			if (!/^(?:0|[1-9]\d*)$/u.test(value)) return usageError();
			afterRevision = Number(value);
			if (!Number.isSafeInteger(afterRevision)) return usageError();
		} else if (option === 'cursor') {
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
		} else if (option === 'rows') {
			if (!/^[1-9]\d{0,3}$/u.test(value)) return usageError();
			rows = Number(value);
			if (rows > 1_000) return usageError();
		} else if (option === 'tab') {
			if (value.length > 512) return usageError();
			tabId = value;
		} else if (option === 'target') {
			const parsed = SemanticCommandLocalEndpointIdSchema.safeParse(value);
			if (!parsed.success) return usageError();
			endpointId = parsed.data;
		} else if (option === 'timeout') {
			if (!/^[1-9]\d*$/u.test(value)) return usageError();
			timeoutMilliseconds = Number(value);
			if (!Number.isSafeInteger(timeoutMilliseconds) || timeoutMilliseconds > 3_600_000) {
				return usageError();
			}
		} else if (option === 'until') {
			const parsed = AgentWaitConditionSchema.safeParse(value);
			if (!parsed.success || until.includes(parsed.data)) return usageError();
			until.push(parsed.data);
		} else {
			if (value.length > 512) return usageError();
			workspaceId = value;
		}
	}
	return {
		kind: 'collected',
		options: {
			afterRevision,
			all,
			cursor,
			detach,
			directoryId,
			endpointId,
			inputFile,
			limit,
			output,
			projectId,
			requestIdentity,
			rows,
			standalone,
			tabId,
			timeoutMilliseconds,
			until,
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
