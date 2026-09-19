import { Buffer } from 'node:buffer';

import {
	SEMANTIC_COMMAND_PROTOCOL_VERSION,
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandJsonValueSchema,
	SemanticCommandRequestIdentitySchema,
	type SemanticCommandCoordinatorGeneration,
	type SemanticCommandExecutionCancellationResult,
	type SemanticCommandExecutionLookupResult,
	type SemanticCommandExecutionReference,
	type SemanticCommandExecutionSnapshot,
	type SemanticCommandInvocationRejectionCode,
	type SemanticCommandJsonValue,
	type SemanticCommandRequestIdentity,
	type SemanticCommandTerminalResult,
} from '@orchbee/semantic-command-protocol';
import {
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES,
	type SemanticCommandLocalEndpointId,
} from '@orchbee/semantic-command-protocol/local-http';

import {
	type SemanticCommandClient,
	type SemanticCommandClientErrorCode,
	type SemanticCommandClientResult,
} from '../client';
import {
	parseSemanticCommandCliArguments,
	type SemanticCommandCliCommand,
	type SemanticCommandCliOwnedOperation,
} from './parseSemanticCommandCliArguments';
import { renderSemanticCommandCliCompletion } from './renderSemanticCommandCliCompletion';
import type { SemanticCommandCliLocale } from './semantic-command-cli-grammar';
import { renderSemanticCommandCliHelp } from './semantic-command-cli-help';
import {
	cliErrorResult,
	cliFailureResult,
	cliProtocolResult,
	type SemanticCommandCliRunResult,
} from './semantic-command-cli-output';
import packageJson from '../../package.json';
import orchbeeSkill from '../../skills/orchbee/SKILL.md?raw';

export interface SemanticCommandCliConnection {
	readonly client: SemanticCommandClient;
	readonly coordinatorGeneration: SemanticCommandCoordinatorGeneration;
}

export type SemanticCommandCliConnectionResult =
	| {
			readonly code: 'coordinator-ambiguous' | 'coordinator-unavailable';
			readonly kind: 'error';
	  }
	| ({ readonly kind: 'connected' } & SemanticCommandCliConnection);

export interface SemanticCommandCliDependencies {
	readonly connect: (
		endpointId: SemanticCommandLocalEndpointId | undefined,
	) => Promise<SemanticCommandCliConnectionResult>;
	readonly createRequestIdentity: () => string;
	readonly locale?: SemanticCommandCliLocale | undefined;
	readonly readInput: (source: string) => Promise<Uint8Array>;
	readonly signal?: AbortSignal | undefined;
}

type TerminalExecutionSnapshot = Extract<
	SemanticCommandExecutionSnapshot,
	{ readonly phase: 'terminal' }
>;

type ExecutionReferenceResolution =
	| {
			readonly code: SemanticCommandClientErrorCode | 'execution-reference-ambiguous';
			readonly kind: 'error';
	  }
	| { readonly kind: 'missing'; readonly lookup: SemanticCommandExecutionLookupResult }
	| {
			readonly kind: 'resolved';
			readonly lookup: Extract<SemanticCommandExecutionLookupResult, { readonly kind: 'found' }>;
			readonly reference: SemanticCommandExecutionReference;
	  };

type WaitResult =
	| SemanticCommandClientResult<SemanticCommandExecutionLookupResult>
	| { readonly code: 'outcome-unknown-after-restart'; readonly kind: 'unknown' };

type SemanticCommandCliCoordinatorCommand = Exclude<
	SemanticCommandCliCommand,
	| { readonly kind: 'skill' }
	| { readonly kind: 'help' }
	| { readonly kind: 'version' }
	| { readonly kind: 'completion' }
>;

type SemanticCommandCliInvocationCommand = Extract<
	SemanticCommandCliCoordinatorCommand,
	{ readonly detach: boolean }
>;

const inputDecoder = new TextDecoder('utf-8', { fatal: true });

export async function runSemanticCommandCli(
	arguments_: readonly string[],
	dependencies: SemanticCommandCliDependencies,
): Promise<SemanticCommandCliRunResult> {
	const parsed = parseSemanticCommandCliArguments(arguments_);
	if (parsed.kind === 'error') return cliErrorResult(null, parsed.code);
	const command = parsed.command;
	if (command.kind === 'skill') return skillResult();
	if (command.kind === 'help') {
		return {
			exitCode: 0,
			stderr: '',
			stdout: renderSemanticCommandCliHelp(command.topic, dependencies.locale ?? 'en-US'),
		};
	}
	if (command.kind === 'version') {
		return { exitCode: 0, stderr: '', stdout: `orchbee-cli ${packageJson.version}\n` };
	}
	if (command.kind === 'completion') {
		return {
			exitCode: 0,
			stderr: '',
			stdout: renderSemanticCommandCliCompletion(command.shell),
		};
	}
	try {
		switch (command.kind) {
			case 'status':
				return runStatus(command, dependencies);
			case 'operation-list':
				return runOperationList(command, dependencies);
			case 'operation-describe':
				return runOperationDescribe(command, dependencies);
			case 'operation-invoke':
				return runOperationInvoke(command, dependencies);
			case 'project-list':
			case 'directory-list':
			case 'workspace-list':
			case 'tab-list':
			case 'panel-list':
			case 'terminal-list':
			case 'terminal-read':
			case 'terminal-create':
			case 'agent-list':
			case 'agent-changes':
			case 'agent-get':
			case 'agent-explain':
			case 'agent-wait':
				return runOwnedOperationInvoke(command, dependencies);
			case 'execution-get':
				return runExecutionGet(command, dependencies);
			case 'execution-wait':
				return runExecutionWait(command, dependencies);
			case 'execution-cancel':
				return runExecutionCancel(command, dependencies);
		}
	} catch {
		return cliFailureResult(commandName(command), 'cli-internal-failure', 6, command.output);
	}
}

async function runStatus(
	command: Extract<SemanticCommandCliCommand, { readonly kind: 'status' }>,
	dependencies: SemanticCommandCliDependencies,
): Promise<SemanticCommandCliRunResult> {
	const connected = await connect(command, dependencies);
	if (isCliRunResult(connected)) return connected;
	const result = await connected.client.readStatus();
	return clientReadResult(command, result, 0);
}

async function runOperationList(
	command: Extract<SemanticCommandCliCommand, { readonly kind: 'operation-list' }>,
	dependencies: SemanticCommandCliDependencies,
): Promise<SemanticCommandCliRunResult> {
	const connected = await connect(command, dependencies);
	if (isCliRunResult(connected)) return connected;
	const result = await connected.client.listOperations();
	return clientReadResult(command, result, 0);
}

async function runOperationDescribe(
	command: Extract<SemanticCommandCliCommand, { readonly kind: 'operation-describe' }>,
	dependencies: SemanticCommandCliDependencies,
): Promise<SemanticCommandCliRunResult> {
	const connected = await connect(command, dependencies);
	if (isCliRunResult(connected)) return connected;
	const result = await connected.client.describeOperation(command.operationId);
	if (result.kind === 'error') return clientFailure(command, result.code);
	return cliProtocolResult(
		commandName(command),
		result.value,
		result.value.kind === 'operation-unavailable' ? 3 : 0,
		command.output,
	);
}

async function runOperationInvoke(
	command: Extract<SemanticCommandCliCommand, { readonly kind: 'operation-invoke' }>,
	dependencies: SemanticCommandCliDependencies,
): Promise<SemanticCommandCliRunResult> {
	const input = await readInvocationInput(command, dependencies);
	if (isCliRunResult(input)) return input;
	return runInvocation(command, input, command.operationId, dependencies);
}

async function runOwnedOperationInvoke(
	command: Exclude<SemanticCommandCliInvocationCommand, { readonly kind: 'operation-invoke' }>,
	dependencies: SemanticCommandCliDependencies,
): Promise<SemanticCommandCliRunResult> {
	return runInvocation(
		command,
		command.input,
		command.operation.id,
		dependencies,
		command.operation,
	);
}

async function runInvocation(
	command: SemanticCommandCliInvocationCommand,
	input: SemanticCommandJsonValue,
	operationId: Parameters<SemanticCommandClient['describeOperation']>[0],
	dependencies: SemanticCommandCliDependencies,
	expectedOperation?: SemanticCommandCliOwnedOperation | undefined,
): Promise<SemanticCommandCliRunResult> {
	const requestIdentity = createRequestIdentity(command, dependencies);
	if (isCliRunResult(requestIdentity)) return requestIdentity;
	const connected = await connect(command, dependencies, requestIdentity);
	if (isCliRunResult(connected)) return connected;
	const described = await connected.client.describeOperation(operationId);
	if (described.kind === 'error') return clientFailure(command, described.code, requestIdentity);
	if (described.value.kind === 'operation-unavailable') {
		return cliProtocolResult(
			commandName(command),
			described.value,
			3,
			command.output,
			requestIdentity,
		);
	}
	if (
		expectedOperation !== undefined &&
		!operationDescriptorsMatch(described.value.operation, expectedOperation)
	) {
		return cliFailureResult(
			commandName(command),
			'operation-version-unsupported',
			6,
			command.output,
			requestIdentity,
		);
	}
	const request = SemanticCommandInvocationRequestSchema.parse({
		input,
		operationId,
		operationSchemaVersion: described.value.operation.schemaVersion,
		protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
		requestIdentity,
	});
	if (
		Buffer.byteLength(JSON.stringify(request), 'utf8') >
		SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES
	) {
		return cliFailureResult(
			commandName(command),
			'input-capacity-exceeded',
			2,
			command.output,
			requestIdentity,
		);
	}
	const invoked = await connected.client.invoke(request);
	if (invoked.kind === 'error') {
		if (
			invoked.code === 'transport-unavailable' ||
			invoked.code === 'coordinator-generation-mismatch'
		) {
			return recoverInvocationObservation(
				command,
				dependencies,
				connected,
				requestIdentity,
				invoked.code,
			);
		}
		return clientFailure(command, invoked.code, requestIdentity);
	}
	if (invoked.value.kind === 'rejected') {
		return cliProtocolResult(
			commandName(command),
			invoked.value,
			exitCodeForInvocationRejection(invoked.value.code),
			command.output,
			requestIdentity,
		);
	}
	if (command.detach) {
		return cliProtocolResult(
			commandName(command),
			invoked.value,
			0,
			command.output,
			requestIdentity,
		);
	}
	if (invoked.value.execution.phase === 'terminal') {
		return terminalLookupResult(command, invoked.value.execution, requestIdentity);
	}
	const waited = await waitAcrossGeneration(command, dependencies, connected, {
		kind: 'request',
		requestIdentity,
	});
	if (waited.kind === 'unknown') {
		return cliFailureResult(commandName(command), waited.code, 6, command.output, requestIdentity);
	}
	if (waited.kind === 'error') return clientFailure(command, waited.code, requestIdentity);
	return executionLookupResult(command, waited.value, requestIdentity);
}

function operationDescriptorsMatch(
	actual: SemanticCommandCliOwnedOperation,
	expected: SemanticCommandCliOwnedOperation,
): boolean {
	return (
		actual.confirmation === expected.confirmation &&
		actual.id === expected.id &&
		actual.input.name === expected.input.name &&
		actual.input.schemaVersion === expected.input.schemaVersion &&
		actual.kind === expected.kind &&
		actual.output.name === expected.output.name &&
		actual.output.schemaVersion === expected.output.schemaVersion &&
		actual.retry === expected.retry &&
		actual.schemaVersion === expected.schemaVersion
	);
}

async function recoverInvocationObservation(
	command: SemanticCommandCliInvocationCommand,
	dependencies: SemanticCommandCliDependencies,
	connection: SemanticCommandCliConnection,
	requestIdentity: SemanticCommandRequestIdentity,
	originalCode: SemanticCommandClientErrorCode,
): Promise<SemanticCommandCliRunResult> {
	let reconnected: SemanticCommandCliConnectionResult;
	try {
		reconnected = await dependencies.connect(command.endpointId);
	} catch {
		return clientFailure(command, originalCode, requestIdentity);
	}
	if (reconnected.kind === 'error') return clientFailure(command, originalCode, requestIdentity);
	if (reconnected.coordinatorGeneration !== connection.coordinatorGeneration) {
		return cliFailureResult(
			commandName(command),
			'outcome-unknown-after-restart',
			6,
			command.output,
			requestIdentity,
		);
	}
	const reference = { kind: 'request', requestIdentity } as const;
	const observed = command.detach
		? await reconnected.client.getExecution(reference)
		: await reconnected.client.waitForExecution(
				reference,
				dependencies.signal === undefined ? {} : { signal: dependencies.signal },
			);
	if (observed.kind === 'error') {
		if (observed.code === 'coordinator-generation-mismatch') {
			return cliFailureResult(
				commandName(command),
				'outcome-unknown-after-restart',
				6,
				command.output,
				requestIdentity,
			);
		}
		return clientFailure(command, observed.code, requestIdentity);
	}
	return executionLookupResult(command, observed.value, requestIdentity);
}

async function runExecutionGet(
	command: Extract<SemanticCommandCliCommand, { readonly kind: 'execution-get' }>,
	dependencies: SemanticCommandCliDependencies,
): Promise<SemanticCommandCliRunResult> {
	const connected = await connect(command, dependencies);
	if (isCliRunResult(connected)) return connected;
	const resolution = await resolveExecutionReference(connected.client, command.referenceIdentity);
	if (resolution.kind === 'error') return resolutionFailure(command, resolution.code);
	return executionLookupResult(command, resolution.lookup);
}

async function runExecutionWait(
	command: Extract<SemanticCommandCliCommand, { readonly kind: 'execution-wait' }>,
	dependencies: SemanticCommandCliDependencies,
): Promise<SemanticCommandCliRunResult> {
	const connected = await connect(command, dependencies);
	if (isCliRunResult(connected)) return connected;
	const resolution = await resolveExecutionReference(connected.client, command.referenceIdentity);
	if (resolution.kind === 'error') return resolutionFailure(command, resolution.code);
	if (resolution.kind === 'missing' || resolution.lookup.execution.phase === 'terminal') {
		return executionLookupResult(command, resolution.lookup);
	}
	const waited = await waitAcrossGeneration(command, dependencies, connected, resolution.reference);
	if (waited.kind === 'unknown') {
		return cliFailureResult(commandName(command), waited.code, 6, command.output);
	}
	if (waited.kind === 'error') return clientFailure(command, waited.code);
	return executionLookupResult(command, waited.value);
}

async function runExecutionCancel(
	command: Extract<SemanticCommandCliCommand, { readonly kind: 'execution-cancel' }>,
	dependencies: SemanticCommandCliDependencies,
): Promise<SemanticCommandCliRunResult> {
	const connected = await connect(command, dependencies);
	if (isCliRunResult(connected)) return connected;
	const resolution = await resolveExecutionReference(connected.client, command.referenceIdentity);
	if (resolution.kind === 'error') return resolutionFailure(command, resolution.code);
	if (resolution.kind === 'missing') return executionLookupResult(command, resolution.lookup);
	const cancelled = await connected.client.cancelExecution(resolution.reference);
	if (cancelled.kind === 'error') return clientFailure(command, cancelled.code);
	return cancellationResult(command, cancelled.value);
}

async function connect(
	command: SemanticCommandCliCoordinatorCommand,
	dependencies: SemanticCommandCliDependencies,
	requestIdentity?: SemanticCommandRequestIdentity | undefined,
): Promise<SemanticCommandCliConnection | SemanticCommandCliRunResult> {
	let result: SemanticCommandCliConnectionResult;
	try {
		result = await dependencies.connect(command.endpointId);
	} catch {
		return cliFailureResult(
			commandName(command),
			'coordinator-unavailable',
			6,
			command.output,
			requestIdentity,
		);
	}
	return result.kind === 'connected'
		? result
		: cliFailureResult(commandName(command), result.code, 6, command.output, requestIdentity);
}

async function readInvocationInput(
	command: Extract<SemanticCommandCliCommand, { readonly kind: 'operation-invoke' }>,
	dependencies: SemanticCommandCliDependencies,
): Promise<SemanticCommandJsonValue | SemanticCommandCliRunResult> {
	let bytes: Uint8Array;
	try {
		bytes = await dependencies.readInput(command.inputFile);
	} catch {
		return cliFailureResult(commandName(command), 'input-unavailable', 2, command.output);
	}
	if (
		bytes.byteLength === 0 ||
		bytes.byteLength > SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES
	) {
		return cliFailureResult(commandName(command), 'input-capacity-exceeded', 2, command.output);
	}
	let candidate: unknown;
	try {
		candidate = JSON.parse(inputDecoder.decode(bytes));
	} catch {
		return cliFailureResult(commandName(command), 'input-json-invalid', 2, command.output);
	}
	const parsed = SemanticCommandJsonValueSchema.safeParse(candidate);
	return parsed.success
		? parsed.data
		: cliFailureResult(commandName(command), 'input-json-invalid', 2, command.output);
}

function createRequestIdentity(
	command: SemanticCommandCliInvocationCommand,
	dependencies: SemanticCommandCliDependencies,
): SemanticCommandRequestIdentity | SemanticCommandCliRunResult {
	let candidate: string;
	try {
		candidate = command.requestIdentity ?? dependencies.createRequestIdentity();
	} catch {
		return cliFailureResult(commandName(command), 'request-identity-invalid', 2, command.output);
	}
	const parsed = SemanticCommandRequestIdentitySchema.safeParse(candidate);
	return parsed.success
		? parsed.data
		: cliFailureResult(commandName(command), 'request-identity-invalid', 2, command.output);
}

async function resolveExecutionReference(
	client: SemanticCommandClient,
	identity: string,
): Promise<ExecutionReferenceResolution> {
	const requestIdentity = SemanticCommandRequestIdentitySchema.parse(identity);
	const executionIdentity = SemanticCommandExecutionIdentitySchema.parse(identity);
	const requestReference = { kind: 'request', requestIdentity } as const;
	const executionReference = { kind: 'execution', executionIdentity } as const;
	const [requestLookup, executionLookup] = await Promise.all([
		client.getExecution(requestReference),
		client.getExecution(executionReference),
	]);
	if (requestLookup.kind === 'error') return requestLookup;
	if (executionLookup.kind === 'error') return executionLookup;
	const requestFound = requestLookup.value.kind === 'found' ? requestLookup.value : null;
	const executionFound = executionLookup.value.kind === 'found' ? executionLookup.value : null;
	if (
		requestFound !== null &&
		executionFound !== null &&
		requestFound.execution.executionIdentity !== executionFound.execution.executionIdentity
	) {
		return { code: 'execution-reference-ambiguous', kind: 'error' };
	}
	if (requestFound !== null) {
		return {
			kind: 'resolved',
			lookup: requestFound,
			reference: {
				executionIdentity: requestFound.execution.executionIdentity,
				kind: 'execution',
			},
		};
	}
	if (executionFound !== null) {
		return { kind: 'resolved', lookup: executionFound, reference: executionReference };
	}
	return { kind: 'missing', lookup: requestLookup.value };
}

async function waitAcrossGeneration(
	command: SemanticCommandCliCoordinatorCommand,
	dependencies: SemanticCommandCliDependencies,
	connection: SemanticCommandCliConnection,
	reference: SemanticCommandExecutionReference,
): Promise<WaitResult> {
	const waitOptions = dependencies.signal === undefined ? {} : { signal: dependencies.signal };
	const first = await connection.client.waitForExecution(reference, waitOptions);
	if (
		first.kind === 'ok' ||
		(first.code !== 'transport-unavailable' && first.code !== 'coordinator-generation-mismatch')
	) {
		return first;
	}
	let reconnected: SemanticCommandCliConnectionResult;
	try {
		reconnected = await dependencies.connect(command.endpointId);
	} catch {
		return first;
	}
	if (reconnected.kind === 'error') return first;
	if (reconnected.coordinatorGeneration !== connection.coordinatorGeneration) {
		return { code: 'outcome-unknown-after-restart', kind: 'unknown' };
	}
	const second = await reconnected.client.waitForExecution(reference, waitOptions);
	return second.kind === 'error' && second.code === 'coordinator-generation-mismatch'
		? { code: 'outcome-unknown-after-restart', kind: 'unknown' }
		: second;
}

function executionLookupResult(
	command: SemanticCommandCliCoordinatorCommand,
	lookup: SemanticCommandExecutionLookupResult,
	requestIdentity?: SemanticCommandRequestIdentity | undefined,
): SemanticCommandCliRunResult {
	if (lookup.kind === 'execution-receipt-unavailable') {
		return cliProtocolResult(commandName(command), lookup, 6, command.output, requestIdentity);
	}
	return cliProtocolResult(
		commandName(command),
		lookup,
		lookup.execution.phase === 'terminal' ? exitCodeForTerminalResult(lookup.execution.result) : 0,
		command.output,
		requestIdentity,
	);
}

function terminalLookupResult(
	command: SemanticCommandCliCoordinatorCommand,
	execution: TerminalExecutionSnapshot,
	requestIdentity: SemanticCommandRequestIdentity,
): SemanticCommandCliRunResult {
	return executionLookupResult(command, { execution, kind: 'found' }, requestIdentity);
}

function cancellationResult(
	command: SemanticCommandCliCoordinatorCommand,
	result: SemanticCommandExecutionCancellationResult,
): SemanticCommandCliRunResult {
	return cliProtocolResult(
		commandName(command),
		result,
		result.kind === 'execution-receipt-unavailable' ? 6 : 0,
		command.output,
	);
}

function clientReadResult<Value>(
	command: SemanticCommandCliCoordinatorCommand,
	result: SemanticCommandClientResult<Value>,
	exitCode: SemanticCommandCliRunResult['exitCode'],
): SemanticCommandCliRunResult {
	return result.kind === 'error'
		? clientFailure(command, result.code)
		: cliProtocolResult(commandName(command), result.value, exitCode, command.output);
}

function clientFailure(
	command: SemanticCommandCliCoordinatorCommand,
	code: SemanticCommandClientErrorCode,
	requestIdentity?: SemanticCommandRequestIdentity | undefined,
): SemanticCommandCliRunResult {
	return cliFailureResult(
		commandName(command),
		code,
		code === 'invalid-request' ? 2 : 6,
		command.output,
		requestIdentity,
	);
}

function resolutionFailure(
	command: SemanticCommandCliCoordinatorCommand,
	code: SemanticCommandClientErrorCode | 'execution-reference-ambiguous',
): SemanticCommandCliRunResult {
	return code === 'execution-reference-ambiguous'
		? cliFailureResult(commandName(command), code, 6, command.output)
		: clientFailure(command, code);
}

function exitCodeForInvocationRejection(
	code: SemanticCommandInvocationRejectionCode,
): SemanticCommandCliRunResult['exitCode'] {
	switch (code) {
		case 'invalid-request':
		case 'input-invalid':
			return 2;
		case 'caller-unauthorized':
		case 'operation-confirmation-required':
		case 'operation-unavailable':
		case 'request-identity-conflict':
			return 3;
		case 'coordinator-unavailable':
		case 'execution-capacity-exhausted':
		case 'operation-version-unsupported':
			return 6;
	}
}

function exitCodeForTerminalResult(
	result: SemanticCommandTerminalResult,
): SemanticCommandCliRunResult['exitCode'] {
	switch (result.kind) {
		case 'completed':
		case 'unchanged':
			return 0;
		case 'blocked':
		case 'unsupported':
			return 3;
		case 'cancelled':
			return 4;
		case 'failed':
			return 5;
	}
}

function commandName(command: SemanticCommandCliCoordinatorCommand): string {
	return command.kind.replaceAll('-', '.');
}

function skillResult(): SemanticCommandCliRunResult {
	return {
		exitCode: 0,
		stderr: '',
		stdout: orchbeeSkill.endsWith('\n') ? orchbeeSkill : `${orchbeeSkill}\n`,
	};
}

function isCliRunResult(value: unknown): value is SemanticCommandCliRunResult {
	return typeof value === 'object' && value !== null && 'exitCode' in value;
}
