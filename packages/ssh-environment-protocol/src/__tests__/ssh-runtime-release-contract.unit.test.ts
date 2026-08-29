import { describe, expect, it } from 'vitest';

import {
	SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_RUNTIME_RELEASE_DIRECTORY_NAME,
	SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME,
	SSH_RUNTIME_RELEASE_PLATFORMS,
	SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME,
	SSH_RUNTIME_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
	SshRuntimeReleaseManifestSchema,
	SshRuntimeReleaseTrustDescriptorSchema,
	SshRuntimeReleaseUnsignedManifestSchema,
	createSshRuntimeManifestSigningPayload,
	encodeSshRuntimeReleaseManifest,
	encodeSshRuntimeReleaseTrustDescriptor,
} from '../ssh-runtime-release-contract';

const unsignedManifest = SshRuntimeReleaseUnsignedManifestSchema.parse({
	schemaVersion: 1,
	runtimeVersion: '1.2.3-beta.4',
	artifacts: [
		{ platform: 'darwin-arm64', sizeBytes: 11, sha256: '1'.repeat(64) },
		{ platform: 'linux-x64', sizeBytes: 22, sha256: '2'.repeat(64) },
	],
});

describe('SSH Runtime release contract', () => {
	it('fixes one canonical platform matrix and bundle layout', () => {
		expect(SSH_RUNTIME_RELEASE_PLATFORMS).toEqual(['darwin-arm64', 'linux-x64']);
		expect(SSH_RUNTIME_RELEASE_DIRECTORY_NAME).toBe('ssh-environment-runtime-release-v1');
		expect(SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME).toBe('manifest.json');
		expect(SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME).toBe('manifest-signing-payload.json');
		expect(SSH_RUNTIME_RELEASE_TRUST_DESCRIPTOR_FILE_NAME).toBe(
			'ssh-environment-runtime-release-trust-v1.json',
		);
		expect(SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS).toEqual({
			'darwin-arm64': 'artifacts/darwin-arm64/orchbee-ssh-environment-runtime',
			'linux-x64': 'artifacts/linux-x64/orchbee-ssh-environment-runtime',
		});
	});

	it('encodes exact signing, signed-manifest, and trust-descriptor bytes', () => {
		const signature = 'S'.repeat(86);
		const manifest = SshRuntimeReleaseManifestSchema.parse({ ...unsignedManifest, signature });
		const descriptor = SshRuntimeReleaseTrustDescriptorSchema.parse({
			schemaVersion: 1,
			signingKeyId: `ed25519-spki-sha256:${'3'.repeat(64)}`,
			publicKeySpki: 'P'.repeat(59),
			manifestSha256: '4'.repeat(64),
		});

		expect(new TextDecoder().decode(createSshRuntimeManifestSigningPayload(unsignedManifest))).toBe(
			`{"schemaVersion":1,"runtimeVersion":"1.2.3-beta.4","artifacts":[{"platform":"darwin-arm64","sizeBytes":11,"sha256":"${'1'.repeat(64)}"},{"platform":"linux-x64","sizeBytes":22,"sha256":"${'2'.repeat(64)}"}]}`,
		);
		expect(new TextDecoder().decode(encodeSshRuntimeReleaseManifest(manifest))).toBe(
			`{"schemaVersion":1,"runtimeVersion":"1.2.3-beta.4","artifacts":[{"platform":"darwin-arm64","sizeBytes":11,"sha256":"${'1'.repeat(64)}"},{"platform":"linux-x64","sizeBytes":22,"sha256":"${'2'.repeat(64)}"}],"signature":"${signature}"}\n`,
		);
		expect(new TextDecoder().decode(encodeSshRuntimeReleaseTrustDescriptor(descriptor))).toBe(
			`{"schemaVersion":1,"signingKeyId":"ed25519-spki-sha256:${'3'.repeat(64)}","publicKeySpki":"${'P'.repeat(59)}","manifestSha256":"${'4'.repeat(64)}"}\n`,
		);
	});

	it('rejects reordered, incomplete, and expanded release records', () => {
		expect(
			SshRuntimeReleaseUnsignedManifestSchema.safeParse({
				...unsignedManifest,
				artifacts: [...unsignedManifest.artifacts].reverse(),
			}).success,
		).toBe(false);
		expect(
			SshRuntimeReleaseUnsignedManifestSchema.safeParse({
				...unsignedManifest,
				artifacts: [unsignedManifest.artifacts[0]],
			}).success,
		).toBe(false);
		expect(
			SshRuntimeReleaseManifestSchema.safeParse({
				...unsignedManifest,
				extra: true,
				signature: 'S'.repeat(86),
			}).success,
		).toBe(false);
		expect(
			SshRuntimeReleaseTrustDescriptorSchema.safeParse({
				schemaVersion: 1,
				signingKeyId: `ed25519-spki-sha256:${'3'.repeat(64)}`,
				publicKeySpki: 'P'.repeat(59),
				manifestSha256: '4'.repeat(64),
				privateKey: 'forbidden',
			}).success,
		).toBe(false);
	});
});
