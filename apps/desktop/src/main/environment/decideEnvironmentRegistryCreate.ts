import {
	DESKTOP_ENVIRONMENT_REGISTRY_LIMITS,
	EnvironmentRecordSchema,
	EnvironmentRegistryDocumentSchema,
	sshEnvironmentTargetKey,
	type EnvironmentRecord,
	type EnvironmentRegistryDocument,
} from './environment-registry-contract';

export type EnvironmentRegistryCreateDecision =
	| {
			readonly document: EnvironmentRegistryDocument;
			readonly environment: EnvironmentRecord;
			readonly kind: 'append';
	  }
	| { readonly environment: EnvironmentRecord; readonly kind: 'existing' }
	| {
			readonly environment: EnvironmentRecord;
			readonly kind: 'conflict';
			readonly reason: 'environment-id' | 'ssh-target';
	  }
	| { readonly kind: 'rejected'; readonly reason: 'capacity' };

export function decideEnvironmentRegistryCreate(
	document: EnvironmentRegistryDocument,
	requestedRecord: EnvironmentRecord,
): EnvironmentRegistryCreateDecision {
	const current = EnvironmentRegistryDocumentSchema.parse(document);
	const requested = EnvironmentRecordSchema.parse(requestedRecord);
	const identityMatch = current.environments.find(
		(environment) => environment.environmentId === requested.environmentId,
	);
	if (identityMatch !== undefined) {
		return environmentRecordsEqual(identityMatch, requested)
			? { environment: identityMatch, kind: 'existing' }
			: { environment: identityMatch, kind: 'conflict', reason: 'environment-id' };
	}
	const targetKey = sshEnvironmentTargetKey(requested.provider.target);
	const targetMatch = current.environments.find(
		(environment) => sshEnvironmentTargetKey(environment.provider.target) === targetKey,
	);
	if (targetMatch !== undefined) {
		return { environment: targetMatch, kind: 'conflict', reason: 'ssh-target' };
	}
	if (current.environments.length >= DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumEnvironmentCount) {
		return { kind: 'rejected', reason: 'capacity' };
	}
	const nextDocument = EnvironmentRegistryDocumentSchema.safeParse({
		environments: [...current.environments, requested],
		revision: current.revision + 1,
		schemaVersion: 1,
	});
	if (!nextDocument.success) return { kind: 'rejected', reason: 'capacity' };
	return { document: nextDocument.data, environment: requested, kind: 'append' };
}

function environmentRecordsEqual(left: EnvironmentRecord, right: EnvironmentRecord): boolean {
	return (
		left.environmentId === right.environmentId &&
		left.name === right.name &&
		left.provider.kind === right.provider.kind &&
		left.provider.runtimeInstanceId === right.provider.runtimeInstanceId &&
		left.provider.target.configurationFilePath === right.provider.target.configurationFilePath &&
		left.provider.target.destination === right.provider.target.destination
	);
}
