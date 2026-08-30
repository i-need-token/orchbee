import { describe, expect, it } from 'vitest';

import { SshEnvironmentTargetReferenceSchema } from '../../ssh-environment';
import {
	EnvironmentRecordSchema,
	EnvironmentRegistryDocumentSchema,
	createEmptyEnvironmentRegistryDocument,
	sshEnvironmentTargetKey,
} from '../environment-registry-contract';

describe('Environment registry contract', () => {
	it('creates the canonical empty append-only document', () => {
		expect(createEmptyEnvironmentRegistryDocument()).toEqual({
			environments: [],
			revision: 0,
			schemaVersion: 2,
		});
	});

	it('derives an exact unambiguous key from configuration path, destination, and options', () => {
		expect(
			sshEnvironmentTargetKey(
				SshEnvironmentTargetReferenceSchema.parse({
					configurationFilePath: '/Users/example/.ssh/config',
					destination: 'build-host',
				}),
			),
		).toBe('["/Users/example/.ssh/config","build-host",[]]');
		expect(
			sshEnvironmentTargetKey(
				SshEnvironmentTargetReferenceSchema.parse({
					configurationFilePath: null,
					destination: 'build-host',
				}),
			),
		).toBe('[null,"build-host",[]]');
		expect(
			sshEnvironmentTargetKey(
				SshEnvironmentTargetReferenceSchema.parse({
					configurationFilePath: null,
					destination: 'build-host',
					options: [
						{ kind: 'port', value: 2222 },
						{ enabled: true, kind: 'forward-agent' },
					],
				}),
			),
		).toBe(
			'[null,"build-host",[{"enabled":true,"kind":"forward-agent"},{"kind":"port","value":2222}]]',
		);
	});

	it('rejects noncanonical identities, names, configuration paths, duplicate records, and extra secrets', () => {
		const valid = EnvironmentRecordSchema.parse({
			environmentId: `environment:${'1'.repeat(32)}`,
			name: 'Build host',
			provider: {
				kind: 'ssh',
				runtimeInstanceId: '2'.repeat(32),
				target: { configurationFilePath: null, destination: 'build-host' },
			},
		});
		expect(
			EnvironmentRecordSchema.safeParse({ ...valid, environmentId: 'build-host' }).success,
		).toBe(false);
		expect(EnvironmentRecordSchema.safeParse({ ...valid, name: ' Build host ' }).success).toBe(
			false,
		);
		expect(EnvironmentRecordSchema.safeParse({ ...valid, name: 'Build\nhost' }).success).toBe(
			false,
		);
		expect(
			EnvironmentRecordSchema.safeParse({
				...valid,
				provider: {
					...valid.provider,
					target: { configurationFilePath: '.ssh/config', destination: 'build-host' },
				},
			}).success,
		).toBe(false);
		expect(
			EnvironmentRecordSchema.safeParse({
				...valid,
				provider: { ...valid.provider, password: 'must-not-persist' },
			}).success,
		).toBe(false);
		expect(
			EnvironmentRegistryDocumentSchema.safeParse({
				environments: [valid, valid],
				revision: 2,
				schemaVersion: 2,
			}).success,
		).toBe(false);
		expect(
			EnvironmentRegistryDocumentSchema.safeParse({
				environments: [valid],
				revision: 2,
				schemaVersion: 2,
			}).success,
		).toBe(false);
	});
});
