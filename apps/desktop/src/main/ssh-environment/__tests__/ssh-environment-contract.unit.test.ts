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
				destination: 'zeroliu@10.144.144.1',
			}),
		).toEqual({ configurationFilePath: null, destination: 'zeroliu@10.144.144.1' });
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
