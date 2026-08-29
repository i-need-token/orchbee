import {
	EnvironmentIdSchema,
	EnvironmentRecordSchema,
	EnvironmentRegistryDocumentSchema,
	type EnvironmentId,
	type EnvironmentRecord,
	type EnvironmentRegistryDocument,
} from './environment-registry-contract';
import { decideEnvironmentRegistryCreate } from './decideEnvironmentRegistryCreate';
import type {
	EnvironmentRegistryStore,
	EnvironmentRegistryStoreCommitResult,
	EnvironmentRegistryStoreUnavailableReason,
} from './FileEnvironmentRegistryStore';

type EnvironmentRegistryUnavailableReason =
	| EnvironmentRegistryStoreUnavailableReason
	| 'commit-indeterminate'
	| 'current-record-missing'
	| 'not-initialized'
	| 'write-failed';

export type EnvironmentRegistryInitializeResult =
	| { readonly document: EnvironmentRegistryDocument; readonly kind: 'ready' }
	| { readonly kind: 'unavailable'; readonly reason: EnvironmentRegistryUnavailableReason };

export type EnvironmentRegistryReadResult<Value> =
	| { readonly kind: 'ready'; readonly value: Value }
	| { readonly kind: 'unavailable'; readonly reason: EnvironmentRegistryUnavailableReason };

export type EnvironmentRegistryCreateResult =
	| {
			readonly environment: EnvironmentRecord;
			readonly kind: 'created' | 'existing';
			readonly revision: number;
	  }
	| {
			readonly environment: EnvironmentRecord;
			readonly kind: 'conflict';
			readonly reason: 'environment-id' | 'ssh-target';
			readonly revision: number;
	  }
	| {
			readonly kind: 'conflict';
			readonly reason: 'stale-revision';
			readonly revision: number;
	  }
	| { readonly kind: 'indeterminate'; readonly reason: 'durability-unconfirmed' }
	| { readonly kind: 'rejected'; readonly reason: 'capacity' }
	| { readonly kind: 'unavailable'; readonly reason: EnvironmentRegistryUnavailableReason };

export class DesktopEnvironmentRegistry {
	#document: EnvironmentRegistryDocument | undefined;
	#operationTail = Promise.resolve();
	readonly #store: EnvironmentRegistryStore;
	#unavailableReason: EnvironmentRegistryUnavailableReason = 'not-initialized';

	constructor(store: EnvironmentRegistryStore) {
		this.#store = store;
	}

	create(environment: EnvironmentRecord): Promise<EnvironmentRegistryCreateResult> {
		const canonical = EnvironmentRecordSchema.parse(environment);
		return this.#enqueue(async () => {
			const current = this.#document;
			if (current === undefined) {
				return { kind: 'unavailable', reason: this.#unavailableReason };
			}
			const decision = decideEnvironmentRegistryCreate(current, canonical);
			if (decision.kind === 'existing') {
				return {
					environment: cloneEnvironmentRecord(decision.environment),
					kind: 'existing',
					revision: current.revision,
				};
			}
			if (decision.kind === 'conflict') {
				return {
					environment: cloneEnvironmentRecord(decision.environment),
					kind: 'conflict',
					reason: decision.reason,
					revision: current.revision,
				};
			}
			if (decision.kind === 'rejected') return decision;

			const commit = await this.#store.commit(current.revision, decision.document);
			return this.#settleCommit(commit, decision.document, canonical);
		});
	}

	get(
		environmentId: EnvironmentId,
	): Promise<EnvironmentRegistryReadResult<EnvironmentRecord | null>> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		return this.#enqueue(async () => {
			if (this.#document === undefined) {
				return { kind: 'unavailable', reason: this.#unavailableReason };
			}
			const environment = this.#document.environments.find(
				(candidate) => candidate.environmentId === canonicalId,
			);
			return {
				kind: 'ready',
				value: environment === undefined ? null : cloneEnvironmentRecord(environment),
			};
		});
	}

	initialize(): Promise<EnvironmentRegistryInitializeResult> {
		return this.#enqueue(async () => {
			const result = await this.#store.load();
			if (result.kind === 'unavailable') {
				this.#document = undefined;
				this.#unavailableReason = result.reason;
				return result;
			}
			this.#document = result.document;
			this.#unavailableReason = 'not-initialized';
			return { document: cloneEnvironmentRegistryDocument(result.document), kind: 'ready' };
		});
	}

	list(): Promise<EnvironmentRegistryReadResult<readonly EnvironmentRecord[]>> {
		return this.#enqueue(async () => {
			if (this.#document === undefined) {
				return { kind: 'unavailable', reason: this.#unavailableReason };
			}
			return {
				kind: 'ready',
				value: this.#document.environments.map(cloneEnvironmentRecord),
			};
		});
	}

	#enqueue<Value>(operation: () => Promise<Value>): Promise<Value> {
		const result = this.#operationTail.then(operation);
		this.#operationTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	#settleCommit(
		commit: EnvironmentRegistryStoreCommitResult,
		intendedDocument: EnvironmentRegistryDocument,
		requestedEnvironment: EnvironmentRecord,
	): EnvironmentRegistryCreateResult {
		switch (commit.kind) {
			case 'committed':
				this.#document = intendedDocument;
				return {
					environment: cloneEnvironmentRecord(requestedEnvironment),
					kind: 'created',
					revision: intendedDocument.revision,
				};
			case 'conflict': {
				this.#document = commit.document;
				const refreshedDecision = decideEnvironmentRegistryCreate(
					commit.document,
					requestedEnvironment,
				);
				if (refreshedDecision.kind === 'existing') {
					return {
						environment: cloneEnvironmentRecord(refreshedDecision.environment),
						kind: 'existing',
						revision: commit.document.revision,
					};
				}
				if (refreshedDecision.kind === 'conflict') {
					return {
						environment: cloneEnvironmentRecord(refreshedDecision.environment),
						kind: 'conflict',
						reason: refreshedDecision.reason,
						revision: commit.document.revision,
					};
				}
				if (refreshedDecision.kind === 'rejected') return refreshedDecision;
				return {
					kind: 'conflict',
					reason: 'stale-revision',
					revision: commit.document.revision,
				};
			}
			case 'indeterminate':
				this.#document = undefined;
				this.#unavailableReason = 'commit-indeterminate';
				return commit;
			case 'unavailable':
				if (commit.reason !== 'write-failed') {
					this.#document = undefined;
					this.#unavailableReason = commit.reason;
				}
				return commit;
		}
	}
}

function cloneEnvironmentRecord(environment: EnvironmentRecord): EnvironmentRecord {
	return EnvironmentRecordSchema.parse({
		environmentId: environment.environmentId,
		name: environment.name,
		provider: {
			kind: environment.provider.kind,
			runtimeInstanceId: environment.provider.runtimeInstanceId,
			target: {
				configurationFilePath: environment.provider.target.configurationFilePath,
				destination: environment.provider.target.destination,
			},
		},
	});
}

function cloneEnvironmentRegistryDocument(
	document: EnvironmentRegistryDocument,
): EnvironmentRegistryDocument {
	return EnvironmentRegistryDocumentSchema.parse({
		environments: document.environments.map(cloneEnvironmentRecord),
		revision: document.revision,
		schemaVersion: document.schemaVersion,
	});
}
