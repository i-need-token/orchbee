import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	DESKTOP_ENVIRONMENT_REGISTRY_LIMITS,
	EnvironmentRecordSchema,
	EnvironmentRegistryDocumentSchema,
	createEmptyEnvironmentRegistryDocument,
	sshEnvironmentTargetKey,
	type EnvironmentRecord,
} from '../environment-registry-contract';
import { decideEnvironmentRegistryCreate } from '../decideEnvironmentRegistryCreate';

describe('Environment registry state', () => {
	it('appends in stable order and treats an identical caller identity as an idempotent replay', () => {
		const first = environment(1, 1);
		const second = environment(2, 2);
		const empty = createEmptyEnvironmentRegistryDocument();
		const firstDecision = decideEnvironmentRegistryCreate(empty, first);
		expect(firstDecision.kind).toBe('append');
		if (firstDecision.kind !== 'append') throw new Error('Expected the first append.');
		const secondDecision = decideEnvironmentRegistryCreate(firstDecision.document, second);
		expect(secondDecision.kind).toBe('append');
		if (secondDecision.kind !== 'append') throw new Error('Expected the second append.');

		expect(secondDecision.document).toMatchObject({
			environments: [first, second],
			revision: 2,
			schemaVersion: 2,
		});
		expect(decideEnvironmentRegistryCreate(secondDecision.document, first)).toEqual({
			environment: first,
			kind: 'existing',
		});
	});

	it('distinguishes identity conflicts from exact SSH target conflicts', () => {
		const original = environment(1, 1);
		const firstDecision = decideEnvironmentRegistryCreate(
			createEmptyEnvironmentRegistryDocument(),
			original,
		);
		if (firstDecision.kind !== 'append') throw new Error('Expected the fixture append.');

		expect(decideEnvironmentRegistryCreate(firstDecision.document, environment(1, 2))).toEqual({
			environment: original,
			kind: 'conflict',
			reason: 'environment-id',
		});
		expect(decideEnvironmentRegistryCreate(firstDecision.document, environment(2, 1))).toEqual({
			environment: original,
			kind: 'conflict',
			reason: 'ssh-target',
		});
	});

	it('treats different canonical SSH options as different immutable targets', () => {
		const original = environment(1, 1);
		const firstDecision = decideEnvironmentRegistryCreate(
			createEmptyEnvironmentRegistryDocument(),
			original,
		);
		if (firstDecision.kind !== 'append') throw new Error('Expected the fixture append.');
		const withAgentForwarding = EnvironmentRecordSchema.parse({
			...environment(2, 1),
			provider: {
				...environment(2, 1).provider,
				target: {
					configurationFilePath: null,
					destination: 'build-1',
					options: [{ enabled: true, kind: 'forward-agent' }],
				},
			},
		});

		expect(
			decideEnvironmentRegistryCreate(firstDecision.document, withAgentForwarding),
		).toMatchObject({
			environment: withAgentForwarding,
			kind: 'append',
		});
	});

	it('allows duplicate display names while enforcing the append-only capacity', () => {
		const environments = Array.from(
			{ length: DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumEnvironmentCount },
			(_, index) => environment(index, index, 'Shared name'),
		);
		const full = EnvironmentRegistryDocumentSchema.parse({
			environments,
			revision: environments.length,
			schemaVersion: 2,
		});

		expect(
			decideEnvironmentRegistryCreate(
				full,
				environment(environments.length, environments.length, 'Shared name'),
			),
		).toEqual({ kind: 'rejected', reason: 'capacity' });
	});

	it('rejects aggregate encoded-byte capacity before the record-count limit', () => {
		const environments = Array.from({ length: 32 }, (_, index) => largeEnvironment(index));
		const document = EnvironmentRegistryDocumentSchema.parse({
			environments,
			revision: environments.length,
			schemaVersion: 2,
		});
		const requested = largeEnvironment(environments.length);
		const oversizedDocument = {
			environments: [...document.environments, requested],
			revision: document.revision + 1,
			schemaVersion: 2,
		};

		expect(Buffer.byteLength(JSON.stringify(document))).toBeLessThanOrEqual(
			DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumDocumentBytes,
		);
		expect(Buffer.byteLength(JSON.stringify(oversizedDocument))).toBeGreaterThan(
			DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumDocumentBytes,
		);
		expect(decideEnvironmentRegistryCreate(document, requested)).toEqual({
			kind: 'rejected',
			reason: 'capacity',
		});
		expect(document.environments.length).toBeLessThan(
			DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumEnvironmentCount,
		);
	});

	it('preserves identity, target uniqueness, order, and revision under generated create traces', () => {
		const action = fc.record({
			identity: fc.integer({ max: 7, min: 0 }),
			name: fc.integer({ max: 3, min: 0 }),
			runtime: fc.integer({ max: 7, min: 0 }),
			target: fc.integer({ max: 7, min: 0 }),
		});

		fc.assert(
			fc.property(fc.array(action, { maxLength: 100, minLength: 1 }), (actions) => {
				let document = createEmptyEnvironmentRegistryDocument();
				for (const currentAction of actions) {
					const requested = environment(
						currentAction.identity,
						currentAction.target,
						`Environment ${currentAction.name}`,
						currentAction.runtime,
					);
					const prior = document;
					const decision = decideEnvironmentRegistryCreate(prior, requested);
					document = decision.kind === 'append' ? decision.document : prior;
					expect(
						decision.kind !== 'append' ||
							isDeepStrictEqual(document.environments.slice(0, -1), prior.environments),
					).toBe(true);
					expect(document.revision).toBe(prior.revision + (decision.kind === 'append' ? 1 : 0));
					expect(EnvironmentRegistryDocumentSchema.safeParse(document).success).toBe(true);
					expect(document.revision).toBe(document.environments.length);
					expect(new Set(document.environments.map((item) => item.environmentId)).size).toBe(
						document.environments.length,
					);
					expect(
						new Set(
							document.environments.map((item) => sshEnvironmentTargetKey(item.provider.target)),
						).size,
					).toBe(document.environments.length);
				}
			}),
			{ numRuns: 200, seed: 2_026_08_28 },
		);
	});
});

function environment(
	identity: number,
	target: number,
	name = `Environment ${identity}`,
	runtime = identity,
): EnvironmentRecord {
	return EnvironmentRecordSchema.parse({
		environmentId: `environment:${hexIdentity(identity)}`,
		name,
		provider: {
			kind: 'ssh',
			runtimeInstanceId: hexIdentity(runtime),
			target: { configurationFilePath: null, destination: `build-${target}` },
		},
	});
}

function hexIdentity(value: number): string {
	return value.toString(16).padStart(32, '0');
}

function largeEnvironment(identity: number): EnvironmentRecord {
	const root = path.parse(process.cwd()).root;
	return EnvironmentRecordSchema.parse({
		environmentId: `environment:${hexIdentity(identity)}`,
		name: `Large Environment ${identity}`,
		provider: {
			kind: 'ssh',
			runtimeInstanceId: hexIdentity(identity + 100),
			target: {
				configurationFilePath: path.join(
					root,
					`${identity.toString().padStart(3, '0')}-${'x'.repeat(32_000)}`,
				),
				destination: `large-build-${identity}`,
			},
		},
	});
}
