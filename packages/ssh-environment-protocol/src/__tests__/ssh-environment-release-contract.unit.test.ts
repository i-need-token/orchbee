import { describe, expect, it } from 'vitest';

import {
	SSH_ENVIRONMENT_RELEASE_ARTIFACT_FILE_NAMES,
	SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES,
	SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME,
	SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME,
	SSH_ENVIRONMENT_RELEASE_PLATFORMS,
	SSH_ENVIRONMENT_RELEASE_SIGNING_PAYLOAD_FILE_NAME,
	SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
	SshEnvironmentReleaseManifestSchema,
	SshEnvironmentReleaseTrustDescriptorSchema,
	SshEnvironmentReleaseUnsignedManifestSchema,
	createSshEnvironmentReleaseManifestSigningPayload,
	encodeSshEnvironmentReleaseManifest,
	encodeSshEnvironmentReleaseTrustDescriptor,
} from '../ssh-environment-release-contract';

const unsignedManifest = SshEnvironmentReleaseUnsignedManifestSchema.parse({
	schemaVersion: 2,
	components: [
		{
			role: 'ssh-environment-runtime',
			version: '1.2.3-beta.4',
			artifacts: [
				{ platform: 'darwin-arm64', sizeBytes: 11, sha256: '1'.repeat(64) },
				{ platform: 'linux-x64', sizeBytes: 22, sha256: '2'.repeat(64) },
			],
		},
		{
			role: 'terminal-host',
			version: '4.5.6',
			artifacts: [
				{ platform: 'darwin-arm64', sizeBytes: 33, sha256: '3'.repeat(64) },
				{ platform: 'linux-x64', sizeBytes: 44, sha256: '4'.repeat(64) },
			],
		},
	],
});

describe('SSH Environment release contract', () => {
	it('fixes one canonical component, platform, and bundle matrix', () => {
		expect(SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES).toEqual([
			'ssh-environment-runtime',
			'terminal-host',
		]);
		expect(SSH_ENVIRONMENT_RELEASE_PLATFORMS).toEqual(['darwin-arm64', 'linux-x64']);
		expect(SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME).toBe('ssh-environment-release-v2');
		expect(SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME).toBe('manifest.json');
		expect(SSH_ENVIRONMENT_RELEASE_SIGNING_PAYLOAD_FILE_NAME).toBe('manifest-signing-payload.json');
		expect(SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME).toBe(
			'ssh-environment-release-trust-v2.json',
		);
		expect(SSH_ENVIRONMENT_RELEASE_ARTIFACT_FILE_NAMES).toEqual({
			'ssh-environment-runtime': 'orchbee-ssh-environment-runtime',
			'terminal-host': 'orchbee-terminal-host',
		});
		expect(SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS).toEqual({
			'ssh-environment-runtime': {
				'darwin-arm64': 'artifacts/darwin-arm64/orchbee-ssh-environment-runtime',
				'linux-x64': 'artifacts/linux-x64/orchbee-ssh-environment-runtime',
			},
			'terminal-host': {
				'darwin-arm64': 'artifacts/darwin-arm64/orchbee-terminal-host',
				'linux-x64': 'artifacts/linux-x64/orchbee-terminal-host',
			},
		});
	});

	it('encodes exact signing, signed-manifest, and trust-descriptor bytes', () => {
		const signature = 'S'.repeat(86);
		const manifest = SshEnvironmentReleaseManifestSchema.parse({
			...unsignedManifest,
			signature,
		});
		const descriptor = SshEnvironmentReleaseTrustDescriptorSchema.parse({
			schemaVersion: 2,
			signingKeyId: `ed25519-spki-sha256:${'5'.repeat(64)}`,
			publicKeySpki: 'P'.repeat(59),
			manifestSha256: '6'.repeat(64),
		});

		expect(
			new TextDecoder().decode(createSshEnvironmentReleaseManifestSigningPayload(unsignedManifest)),
		).toBe(
			`{"schemaVersion":2,"components":[{"role":"ssh-environment-runtime","version":"1.2.3-beta.4","artifacts":[{"platform":"darwin-arm64","sizeBytes":11,"sha256":"${'1'.repeat(64)}"},{"platform":"linux-x64","sizeBytes":22,"sha256":"${'2'.repeat(64)}"}]},{"role":"terminal-host","version":"4.5.6","artifacts":[{"platform":"darwin-arm64","sizeBytes":33,"sha256":"${'3'.repeat(64)}"},{"platform":"linux-x64","sizeBytes":44,"sha256":"${'4'.repeat(64)}"}]}]}`,
		);
		expect(new TextDecoder().decode(encodeSshEnvironmentReleaseManifest(manifest))).toBe(
			`{"schemaVersion":2,"components":[{"role":"ssh-environment-runtime","version":"1.2.3-beta.4","artifacts":[{"platform":"darwin-arm64","sizeBytes":11,"sha256":"${'1'.repeat(64)}"},{"platform":"linux-x64","sizeBytes":22,"sha256":"${'2'.repeat(64)}"}]},{"role":"terminal-host","version":"4.5.6","artifacts":[{"platform":"darwin-arm64","sizeBytes":33,"sha256":"${'3'.repeat(64)}"},{"platform":"linux-x64","sizeBytes":44,"sha256":"${'4'.repeat(64)}"}]}],"signature":"${signature}"}\n`,
		);
		expect(new TextDecoder().decode(encodeSshEnvironmentReleaseTrustDescriptor(descriptor))).toBe(
			`{"schemaVersion":2,"signingKeyId":"ed25519-spki-sha256:${'5'.repeat(64)}","publicKeySpki":"${'P'.repeat(59)}","manifestSha256":"${'6'.repeat(64)}"}\n`,
		);
	});

	it('rejects reordered, incomplete, expanded, and oversized release records', () => {
		expect(
			SshEnvironmentReleaseUnsignedManifestSchema.safeParse({
				...unsignedManifest,
				components: [...unsignedManifest.components].reverse(),
			}).success,
		).toBe(false);
		expect(
			SshEnvironmentReleaseUnsignedManifestSchema.safeParse({
				...unsignedManifest,
				components: [unsignedManifest.components[0]],
			}).success,
		).toBe(false);
		expect(
			SshEnvironmentReleaseUnsignedManifestSchema.safeParse({
				...unsignedManifest,
				components: [
					{
						...unsignedManifest.components[0],
						artifacts: [...unsignedManifest.components[0].artifacts].reverse(),
					},
					unsignedManifest.components[1],
				],
			}).success,
		).toBe(false);
		expect(
			SshEnvironmentReleaseManifestSchema.safeParse({
				...unsignedManifest,
				extra: true,
				signature: 'S'.repeat(86),
			}).success,
		).toBe(false);
		expect(
			SshEnvironmentReleaseUnsignedManifestSchema.safeParse({
				...unsignedManifest,
				components: [
					unsignedManifest.components[0],
					{
						...unsignedManifest.components[1],
						artifacts: [
							{
								...unsignedManifest.components[1].artifacts[0],
								sizeBytes: 128 * 1024 * 1024 + 1,
							},
							unsignedManifest.components[1].artifacts[1],
						],
					},
				],
			}).success,
		).toBe(false);
	});
});
