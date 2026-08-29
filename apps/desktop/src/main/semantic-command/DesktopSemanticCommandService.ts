import { Buffer } from 'node:buffer';

import {
	SEMANTIC_COMMAND_PROTOCOL_VERSION,
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandTerminalResultSchema,
	type SemanticCommandCoordinatorGeneration,
	type SemanticCommandCoordinatorStatus,
	type SemanticCommandExecutionCancellationResult,
	type SemanticCommandExecutionIdentity,
	type SemanticCommandExecutionLookupResult,
	type SemanticCommandExecutionReference,
	type SemanticCommandInvocationRejectionCode,
	type SemanticCommandInvocationRequest,
	type SemanticCommandInvocationResult,
	type SemanticCommandOperationDescriptionResult,
	type SemanticCommandOperationId,
	type SemanticCommandOperationList,
	type SemanticCommandTerminalResult,
} from '@orchbee/semantic-command-protocol';

import type {
	DesktopSemanticCommandCaller,
	DesktopSemanticCommandCanonicalInputResult,
	DesktopSemanticCommandCatalog,
	DesktopSemanticCommandHandlerSettlement,
	DesktopSemanticCommandOperation,
} from './DesktopSemanticCommandCatalog';
import {
	DesktopSemanticCommandExecutionLedger,
	type DesktopSemanticCommandExecutionLedgerOptions,
} from './DesktopSemanticCommandExecutionLedger';
import { createSemanticCommandFingerprint } from './createSemanticCommandFingerprint';

interface DesktopSemanticCommandServiceOptions extends DesktopSemanticCommandExecutionLedgerOptions {
	readonly createExecutionIdentity: () => SemanticCommandExecutionIdentity;
	readonly maxExecutionResultBytes?: number;
}

const DEFAULT_SEMANTIC_COMMAND_EXECUTION_RESULT_BYTE_LIMIT = 1_000_000;

function failedResult(code: string): SemanticCommandTerminalResult {
	return SemanticCommandTerminalResultSchema.parse({ kind: 'failed', code });
}

function encodedResultBytes(result: SemanticCommandTerminalResult): number {
	return Buffer.byteLength(JSON.stringify(result), 'utf8');
}

export class DesktopSemanticCommandService {
	readonly #catalog: DesktopSemanticCommandCatalog;
	readonly #controllers = new Map<SemanticCommandExecutionIdentity, AbortController>();
	readonly #coordinatorGeneration: SemanticCommandCoordinatorGeneration;
	readonly #createExecutionIdentity: () => SemanticCommandExecutionIdentity;
	#disposed = false;
	readonly #ledger: DesktopSemanticCommandExecutionLedger;
	readonly #maxExecutionResultBytes: number;

	constructor(
		catalog: DesktopSemanticCommandCatalog,
		options: DesktopSemanticCommandServiceOptions,
	) {
		this.#catalog = catalog;
		this.#coordinatorGeneration = options.coordinatorGeneration;
		this.#createExecutionIdentity = options.createExecutionIdentity;
		this.#ledger = new DesktopSemanticCommandExecutionLedger(options);
		this.#maxExecutionResultBytes = requirePositiveInteger(
			options.maxExecutionResultBytes ?? DEFAULT_SEMANTIC_COMMAND_EXECUTION_RESULT_BYTE_LIMIT,
			'maxExecutionResultBytes',
		);
	}

	cancel(reference: SemanticCommandExecutionReference): SemanticCommandExecutionCancellationResult {
		const result = this.#ledger.requestCancellation(reference);
		if (result.kind === 'cancellation-observed' && result.forwarded) {
			const controller = this.#controllers.get(result.execution.executionIdentity);
			if (controller === undefined) {
				throw new Error('Active semantic command execution has no cancellation owner.');
			}
			controller.abort();
		}
		return result;
	}

	describe(
		caller: DesktopSemanticCommandCaller,
		operationId: SemanticCommandOperationId,
	): SemanticCommandOperationDescriptionResult {
		const operation = this.#catalog.describe(caller, operationId);
		return operation === null
			? {
					protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
					coordinatorGeneration: this.#coordinatorGeneration,
					kind: 'operation-unavailable',
					operationId,
				}
			: {
					protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
					coordinatorGeneration: this.#coordinatorGeneration,
					kind: 'operation-description',
					operation,
				};
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		for (const controller of this.#controllers.values()) controller.abort();
		this.#controllers.clear();
	}

	get(reference: SemanticCommandExecutionReference): SemanticCommandExecutionLookupResult {
		return this.#ledger.lookup(reference);
	}

	invoke(
		caller: DesktopSemanticCommandCaller,
		request: SemanticCommandInvocationRequest,
	): SemanticCommandInvocationResult {
		if (this.#disposed) return this.#reject(request, 'coordinator-unavailable');
		const resolution = this.#catalog.resolve(
			caller,
			request.operationId,
			request.operationSchemaVersion,
		);
		if (resolution.kind !== 'resolved') return this.#reject(request, resolution.kind);
		const operation = resolution.operation;
		let canonicalInput: DesktopSemanticCommandCanonicalInputResult;
		try {
			canonicalInput = operation.canonicalizeInput(request.input);
		} catch {
			return this.#reject(request, 'input-invalid');
		}
		if (canonicalInput.kind === 'rejected') return this.#reject(request, 'input-invalid');
		let authorized = false;
		try {
			authorized = operation.authorize(caller, canonicalInput.input);
		} catch {
			authorized = false;
		}
		if (!authorized) return this.#reject(request, 'caller-unauthorized');
		if (operation.descriptor.confirmation !== 'noninteractive-allowed') {
			return this.#reject(request, 'operation-confirmation-required');
		}

		const canonicalRequest = { ...request, input: canonicalInput.input };
		const fingerprint = createSemanticCommandFingerprint(canonicalRequest);
		const admission = this.#ledger.admit(canonicalRequest, fingerprint, () =>
			SemanticCommandExecutionIdentitySchema.parse(this.#createExecutionIdentity()),
		);
		if (admission.kind === 'conflict') {
			return this.#reject(request, 'request-identity-conflict');
		}
		if (admission.kind === 'capacity-exhausted') {
			return this.#reject(request, 'execution-capacity-exhausted');
		}
		if (admission.kind === 'existing') {
			return { kind: 'accepted', replayed: true, execution: admission.snapshot };
		}

		const controller = new AbortController();
		this.#controllers.set(admission.snapshot.executionIdentity, controller);
		void this.#execute(
			admission.snapshot.executionIdentity,
			canonicalInput.input,
			controller,
			operation.execute,
			operation.validateOutput,
		);
		return { kind: 'accepted', replayed: false, execution: admission.snapshot };
	}

	list(caller: DesktopSemanticCommandCaller): SemanticCommandOperationList {
		return {
			protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
			coordinatorGeneration: this.#coordinatorGeneration,
			kind: 'operation-list',
			operations: [...this.#catalog.list(caller)],
		};
	}

	status(): SemanticCommandCoordinatorStatus {
		return {
			protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
			coordinatorGeneration: this.#coordinatorGeneration,
			kind: 'ready',
		};
	}

	async #execute(
		executionIdentity: SemanticCommandExecutionIdentity,
		input: SemanticCommandInvocationRequest['input'],
		controller: AbortController,
		execute: DesktopSemanticCommandOperation['execute'],
		validateOutput: DesktopSemanticCommandOperation['validateOutput'],
	): Promise<void> {
		let settlement: DesktopSemanticCommandHandlerSettlement;
		try {
			settlement = await execute(input, {
				markCommitted: () => {
					this.#ledger.markCommitted(executionIdentity);
				},
				signal: controller.signal,
			});
		} catch {
			settlement = { effect: 'none', result: failedResult('operation-handler-failed') };
		}
		if (this.#disposed) return;
		if (settlement.effect === 'committed') this.#ledger.markCommitted(executionIdentity);
		let result = this.#validateSettlement(
			settlement,
			validateOutput,
			this.#ledger.isCommitted(executionIdentity),
		);
		let resultBytes = encodedResultBytes(result);
		if (resultBytes > this.#maxExecutionResultBytes) {
			result = failedResult('output-capacity-exceeded');
			resultBytes = encodedResultBytes(result);
		}
		this.#controllers.delete(executionIdentity);
		this.#ledger.complete(executionIdentity, result, resultBytes);
	}

	#reject(
		request: SemanticCommandInvocationRequest,
		code: SemanticCommandInvocationRejectionCode,
	): SemanticCommandInvocationResult {
		return {
			protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
			coordinatorGeneration: this.#coordinatorGeneration,
			kind: 'rejected',
			requestIdentity: request.requestIdentity,
			code,
		};
	}

	#validateSettlement(
		settlement: DesktopSemanticCommandHandlerSettlement,
		validateOutput: DesktopSemanticCommandOperation['validateOutput'],
		committed: boolean,
	): SemanticCommandTerminalResult {
		const parsed = SemanticCommandTerminalResultSchema.safeParse(settlement.result);
		if (!parsed.success) return failedResult('operation-result-invalid');
		const result = parsed.data;
		if (committed !== (settlement.effect === 'committed')) {
			return failedResult('operation-result-invalid');
		}
		if (committed && result.kind !== 'completed') {
			return failedResult('operation-result-invalid');
		}
		if (result.kind !== 'completed' && result.kind !== 'unchanged') return result;
		try {
			return validateOutput(result.output) ? result : failedResult('operation-output-invalid');
		} catch {
			return failedResult('operation-output-invalid');
		}
	}
}

function requirePositiveInteger(value: number, field: string): number {
	if (!Number.isSafeInteger(value) || value <= 0) {
		throw new Error(`${field} must be a positive safe integer.`);
	}
	return value;
}
