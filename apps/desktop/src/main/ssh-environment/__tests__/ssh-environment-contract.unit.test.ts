import { describe, expect, it } from 'vitest';

import {
	SshEnvironmentConnectionIdSchema,
	SshEnvironmentInteractionHandleSchema,
	SshEnvironmentResolvedTargetSchema,
	SshEnvironmentTargetReferenceSchema,
} from '../ssh-environment-contract';

describe('SSH Environment contract', () => {
	it('admits one concrete destination with a canonical nullable configuration path', () => {
		expect(
			SshEnvironmentTargetReferenceSchema.parse({
				configurationFilePath: null,
				destination: 'alice@192.0.2.10',
			}),
		).toEqual({ configurationFilePath: null, destination: 'alice@192.0.2.10' });
	});

	it('canonicalizes supported connection options and rejects duplicate singleton options', () => {
		expect(
			SshEnvironmentTargetReferenceSchema.parse({
				configurationFilePath: null,
				destination: 'alice@192.0.2.10',
				options: [
					{ kind: 'port', value: 2222 },
					{ enabled: true, kind: 'forward-agent' },
				],
			}),
		).toEqual({
			configurationFilePath: null,
			destination: 'alice@192.0.2.10',
			options: [
				{ enabled: true, kind: 'forward-agent' },
				{ kind: 'port', value: 2222 },
			],
		});
		expect(
			SshEnvironmentTargetReferenceSchema.safeParse({
				configurationFilePath: null,
				destination: 'alice@192.0.2.10',
				options: [
					{ kind: 'port', value: 22 },
					{ kind: 'port', value: 2222 },
				],
			}).success,
		).toBe(false);
	});

	it.each(['', 'two hosts', 'host\nname', '-oProxyCommand=bad'])(
		'rejects an unsafe concrete destination %j',
		(destination) => {
			expect(
				SshEnvironmentTargetReferenceSchema.safeParse({
					configurationFilePath: null,
					destination,
				}).success,
			).toBe(false);
		},
	);

	it('rejects malformed connection and interaction identities', () => {
		expect(SshEnvironmentConnectionIdSchema.safeParse('../connection').success).toBe(false);
		expect(SshEnvironmentInteractionHandleSchema.safeParse('prompt with spaces').success).toBe(
			false,
		);
	});

	it('validates the complete secret-free resolved target identity', () => {
		expect(
			SshEnvironmentResolvedTargetSchema.parse({
				destination: 'build-host',
				hostKeyLookupName: 'build.internal.example',
				hostKeyPolicy: 'ask',
				hostname: '10.0.0.8',
				port: 22,
				user: 'builder',
			}),
		).toEqual({
			destination: 'build-host',
			hostKeyLookupName: 'build.internal.example',
			hostKeyPolicy: 'ask',
			hostname: '10.0.0.8',
			port: 22,
			user: 'builder',
		});
	});
});
