import {
	SemanticCommandOperationDescriptorSchema,
	type SemanticCommandJsonValue,
	type SemanticCommandOperationDescriptor,
	type SemanticCommandOperationId,
	type SemanticCommandSchemaVersion,
	type SemanticCommandTerminalResult,
} from '@orchbee/semantic-command-protocol';

export interface DesktopSemanticCommandCaller {
	readonly kind: 'local-user';
}

export type DesktopSemanticCommandCanonicalInputResult =
	| { readonly input: SemanticCommandJsonValue; readonly kind: 'accepted' }
	| { readonly kind: 'rejected' };

interface DesktopSemanticCommandExecutionContext {
	readonly markCommitted: () => void;
	readonly signal: AbortSignal;
}

export type DesktopSemanticCommandHandlerSettlement =
	| {
			readonly effect: 'committed';
			readonly result: Extract<SemanticCommandTerminalResult, { readonly kind: 'completed' }>;
	  }
	| {
			readonly effect: 'none';
			readonly result: SemanticCommandTerminalResult;
	  };

export interface DesktopSemanticCommandOperation {
	readonly authorize: (
		caller: DesktopSemanticCommandCaller,
		input: SemanticCommandJsonValue,
	) => boolean;
	readonly canonicalizeInput: (
		input: SemanticCommandJsonValue,
	) => DesktopSemanticCommandCanonicalInputResult;
	readonly descriptor: SemanticCommandOperationDescriptor;
	readonly execute: (
		input: SemanticCommandJsonValue,
		context: DesktopSemanticCommandExecutionContext,
	) => Promise<DesktopSemanticCommandHandlerSettlement>;
	readonly isVisibleTo: (caller: DesktopSemanticCommandCaller) => boolean;
	readonly validateOutput: (output: SemanticCommandJsonValue) => boolean;
}

export type DesktopSemanticCommandCatalogResolution =
	| { readonly kind: 'operation-unavailable' }
	| { readonly kind: 'operation-version-unsupported' }
	| { readonly kind: 'resolved'; readonly operation: DesktopSemanticCommandOperation };

function compareDescriptors(
	left: DesktopSemanticCommandOperation,
	right: DesktopSemanticCommandOperation,
): number {
	if (left.descriptor.id < right.descriptor.id) return -1;
	if (left.descriptor.id > right.descriptor.id) return 1;
	return 0;
}

function isVisible(
	operation: DesktopSemanticCommandOperation,
	caller: DesktopSemanticCommandCaller,
): boolean {
	try {
		return operation.isVisibleTo(caller);
	} catch {
		return false;
	}
}

export class DesktopSemanticCommandCatalog {
	readonly #operations: readonly DesktopSemanticCommandOperation[];
	readonly #operationsById: ReadonlyMap<
		SemanticCommandOperationId,
		DesktopSemanticCommandOperation
	>;

	constructor(operations: readonly DesktopSemanticCommandOperation[]) {
		const operationsById = new Map<SemanticCommandOperationId, DesktopSemanticCommandOperation>();
		for (const operation of operations) {
			const descriptor = SemanticCommandOperationDescriptorSchema.parse(operation.descriptor);
			if (operationsById.has(descriptor.id)) {
				throw new Error(`Duplicate semantic command operation ${descriptor.id}.`);
			}
			operationsById.set(descriptor.id, { ...operation, descriptor });
		}
		this.#operations = [...operationsById.values()].sort(compareDescriptors);
		this.#operationsById = operationsById;
	}

	describe(
		caller: DesktopSemanticCommandCaller,
		operationId: SemanticCommandOperationId,
	): SemanticCommandOperationDescriptor | null {
		const operation = this.#operationsById.get(operationId);
		return operation !== undefined && isVisible(operation, caller) ? operation.descriptor : null;
	}

	list(caller: DesktopSemanticCommandCaller): readonly SemanticCommandOperationDescriptor[] {
		return this.#operations
			.filter((operation) => isVisible(operation, caller))
			.map((operation) => operation.descriptor);
	}

	resolve(
		caller: DesktopSemanticCommandCaller,
		operationId: SemanticCommandOperationId,
		schemaVersion: SemanticCommandSchemaVersion,
	): DesktopSemanticCommandCatalogResolution {
		const operation = this.#operationsById.get(operationId);
		if (operation === undefined || !isVisible(operation, caller)) {
			return { kind: 'operation-unavailable' };
		}
		if (operation.descriptor.schemaVersion !== schemaVersion) {
			return { kind: 'operation-version-unsupported' };
		}
		return { kind: 'resolved', operation };
	}
}
