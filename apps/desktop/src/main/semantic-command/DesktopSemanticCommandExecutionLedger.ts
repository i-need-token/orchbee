import {
	SEMANTIC_COMMAND_PROTOCOL_VERSION,
	type SemanticCommandCoordinatorGeneration,
	type SemanticCommandExecutionCancellationResult,
	type SemanticCommandExecutionIdentity,
	type SemanticCommandExecutionLookupResult,
	type SemanticCommandExecutionReference,
	type SemanticCommandExecutionSnapshot,
	type SemanticCommandInvocationRequest,
	type SemanticCommandTerminalResult,
} from '@orchbee/semantic-command-protocol';

const DEFAULT_SEMANTIC_COMMAND_ACTIVE_EXECUTION_LIMIT = 32;
const DEFAULT_SEMANTIC_COMMAND_TERMINAL_EXECUTION_LIMIT = 256;
const DEFAULT_SEMANTIC_COMMAND_TERMINAL_RESULT_BYTE_LIMIT = 32 * 1024 * 1024;

export interface DesktopSemanticCommandExecutionLedgerOptions {
	readonly coordinatorGeneration: SemanticCommandCoordinatorGeneration;
	readonly maxActiveExecutions?: number;
	readonly maxTerminalExecutions?: number;
	readonly maxTerminalResultBytes?: number;
}

interface ActiveExecutionRecord {
	committed: boolean;
	readonly fingerprint: string;
	readonly resultBytes: 0;
	snapshot: Extract<SemanticCommandExecutionSnapshot, { readonly phase: 'active' }>;
}

interface TerminalExecutionRecord {
	readonly fingerprint: string;
	readonly resultBytes: number;
	readonly snapshot: Extract<SemanticCommandExecutionSnapshot, { readonly phase: 'terminal' }>;
}

type ExecutionRecord = ActiveExecutionRecord | TerminalExecutionRecord;

function isActiveExecutionRecord(record: ExecutionRecord): record is ActiveExecutionRecord {
	return record.snapshot.phase === 'active';
}

type DesktopSemanticCommandExecutionAdmission =
	| {
			readonly kind: 'admitted';
			readonly snapshot: Extract<SemanticCommandExecutionSnapshot, { readonly phase: 'active' }>;
	  }
	| { readonly kind: 'capacity-exhausted' }
	| { readonly kind: 'conflict' }
	| { readonly kind: 'existing'; readonly snapshot: SemanticCommandExecutionSnapshot };

function requirePositiveInteger(value: number, field: string): number {
	if (!Number.isSafeInteger(value) || value <= 0) {
		throw new Error(`${field} must be a positive safe integer.`);
	}
	return value;
}

export class DesktopSemanticCommandExecutionLedger {
	#activeCount = 0;
	readonly #coordinatorGeneration: SemanticCommandCoordinatorGeneration;
	readonly #executions = new Map<SemanticCommandExecutionIdentity, ExecutionRecord>();
	readonly #maxActiveExecutions: number;
	readonly #maxTerminalExecutions: number;
	readonly #maxTerminalResultBytes: number;
	readonly #requests = new Map<string, SemanticCommandExecutionIdentity>();
	readonly #terminalOrder: SemanticCommandExecutionIdentity[] = [];
	#terminalResultBytes = 0;
	readonly #usedExecutionIdentities = new Set<SemanticCommandExecutionIdentity>();

	constructor(options: DesktopSemanticCommandExecutionLedgerOptions) {
		this.#coordinatorGeneration = options.coordinatorGeneration;
		this.#maxActiveExecutions = requirePositiveInteger(
			options.maxActiveExecutions ?? DEFAULT_SEMANTIC_COMMAND_ACTIVE_EXECUTION_LIMIT,
			'maxActiveExecutions',
		);
		this.#maxTerminalExecutions = requirePositiveInteger(
			options.maxTerminalExecutions ?? DEFAULT_SEMANTIC_COMMAND_TERMINAL_EXECUTION_LIMIT,
			'maxTerminalExecutions',
		);
		this.#maxTerminalResultBytes = requirePositiveInteger(
			options.maxTerminalResultBytes ?? DEFAULT_SEMANTIC_COMMAND_TERMINAL_RESULT_BYTE_LIMIT,
			'maxTerminalResultBytes',
		);
	}

	get activeCount(): number {
		return this.#activeCount;
	}

	get maxTerminalResultBytes(): number {
		return this.#maxTerminalResultBytes;
	}

	get terminalCount(): number {
		return this.#terminalOrder.length;
	}

	get terminalResultBytes(): number {
		return this.#terminalResultBytes;
	}

	admit(
		request: SemanticCommandInvocationRequest,
		fingerprint: string,
		createExecutionIdentity: () => SemanticCommandExecutionIdentity,
	): DesktopSemanticCommandExecutionAdmission {
		const retainedExecutionIdentity = this.#requests.get(request.requestIdentity);
		if (retainedExecutionIdentity !== undefined) {
			const retained = this.#executions.get(retainedExecutionIdentity);
			if (retained === undefined) throw new Error('Semantic command request mapping is corrupt.');
			return retained.fingerprint === fingerprint
				? { kind: 'existing', snapshot: retained.snapshot }
				: { kind: 'conflict' };
		}
		if (this.#activeCount >= this.#maxActiveExecutions) {
			return { kind: 'capacity-exhausted' };
		}

		const executionIdentity = createExecutionIdentity();
		if (this.#usedExecutionIdentities.has(executionIdentity)) {
			throw new Error('Semantic command execution identities must be process-unique.');
		}
		const snapshot = {
			protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
			coordinatorGeneration: this.#coordinatorGeneration,
			executionIdentity,
			requestIdentity: request.requestIdentity,
			operationId: request.operationId,
			operationSchemaVersion: request.operationSchemaVersion,
			phase: 'active',
			cancellationRequested: false,
		} as const;
		this.#usedExecutionIdentities.add(executionIdentity);
		this.#requests.set(request.requestIdentity, executionIdentity);
		this.#executions.set(executionIdentity, {
			committed: false,
			fingerprint,
			resultBytes: 0,
			snapshot,
		});
		this.#activeCount += 1;
		return { kind: 'admitted', snapshot };
	}

	complete(
		executionIdentity: SemanticCommandExecutionIdentity,
		result: SemanticCommandTerminalResult,
		resultBytes: number,
	): SemanticCommandExecutionSnapshot {
		if (!Number.isSafeInteger(resultBytes) || resultBytes < 0) {
			throw new Error('Semantic command result bytes must be a nonnegative safe integer.');
		}
		if (resultBytes > this.#maxTerminalResultBytes) {
			throw new Error('Semantic command result exceeds the ledger byte limit.');
		}
		const active = this.#executions.get(executionIdentity);
		if (active === undefined || active.snapshot.phase !== 'active') {
			throw new Error('Only an active semantic command execution may complete.');
		}
		const snapshot = {
			protocolVersion: active.snapshot.protocolVersion,
			coordinatorGeneration: active.snapshot.coordinatorGeneration,
			executionIdentity: active.snapshot.executionIdentity,
			requestIdentity: active.snapshot.requestIdentity,
			operationId: active.snapshot.operationId,
			operationSchemaVersion: active.snapshot.operationSchemaVersion,
			phase: 'terminal',
			result,
		} as const;
		this.#executions.set(executionIdentity, {
			fingerprint: active.fingerprint,
			resultBytes,
			snapshot,
		});
		this.#activeCount -= 1;
		this.#terminalOrder.push(executionIdentity);
		this.#terminalResultBytes += resultBytes;
		this.#enforceTerminalLimits();
		return snapshot;
	}

	lookup(reference: SemanticCommandExecutionReference): SemanticCommandExecutionLookupResult {
		const record = this.#resolve(reference);
		return record === undefined
			? {
					protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
					coordinatorGeneration: this.#coordinatorGeneration,
					kind: 'execution-receipt-unavailable',
					reference,
				}
			: { kind: 'found', execution: record.snapshot };
	}

	isCommitted(executionIdentity: SemanticCommandExecutionIdentity): boolean {
		const record = this.#executions.get(executionIdentity);
		return record !== undefined && isActiveExecutionRecord(record) && record.committed;
	}

	markCommitted(executionIdentity: SemanticCommandExecutionIdentity): boolean {
		const record = this.#executions.get(executionIdentity);
		if (record === undefined || !isActiveExecutionRecord(record) || record.committed) return false;
		record.committed = true;
		return true;
	}

	requestCancellation(
		reference: SemanticCommandExecutionReference,
	): SemanticCommandExecutionCancellationResult {
		const record = this.#resolve(reference);
		if (record === undefined) {
			return {
				protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
				coordinatorGeneration: this.#coordinatorGeneration,
				kind: 'execution-receipt-unavailable',
				reference,
			};
		}
		if (!isActiveExecutionRecord(record)) {
			return { kind: 'cancellation-observed', forwarded: false, execution: record.snapshot };
		}
		if (record.committed) {
			return { kind: 'cancellation-observed', forwarded: false, execution: record.snapshot };
		}
		if (record.snapshot.cancellationRequested) {
			return { kind: 'cancellation-observed', forwarded: false, execution: record.snapshot };
		}
		record.snapshot = { ...record.snapshot, cancellationRequested: true };
		return { kind: 'cancellation-observed', forwarded: true, execution: record.snapshot };
	}

	#enforceTerminalLimits(): void {
		while (
			this.#terminalOrder.length > this.#maxTerminalExecutions ||
			this.#terminalResultBytes > this.#maxTerminalResultBytes
		) {
			const oldest = this.#terminalOrder.shift();
			if (oldest === undefined) throw new Error('Semantic command terminal order is corrupt.');
			const record = this.#executions.get(oldest);
			if (record === undefined || record.snapshot.phase !== 'terminal') {
				throw new Error('Semantic command terminal record is corrupt.');
			}
			this.#terminalResultBytes -= record.resultBytes;
			this.#executions.delete(oldest);
			this.#requests.delete(record.snapshot.requestIdentity);
		}
	}

	#resolve(reference: SemanticCommandExecutionReference): ExecutionRecord | undefined {
		if (reference.kind === 'execution') return this.#executions.get(reference.executionIdentity);
		const executionIdentity = this.#requests.get(reference.requestIdentity);
		return executionIdentity === undefined ? undefined : this.#executions.get(executionIdentity);
	}
}
