import { createHash, generateKeyPairSync, sign, type KeyObject } from 'node:crypto';

import {
	SSH_RUNTIME_RELEASE_SCHEMA_VERSION,
	SSH_RUNTIME_RELEASE_SIGNING_KEY_ID_PREFIX,
	SSH_RUNTIME_RELEASE_TRUST_SCHEMA_VERSION,
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentRuntimeVersionSchema,
	SshRuntimeReleaseManifestSchema,
	SshRuntimeReleaseTrustDescriptorSchema,
	SshRuntimeReleaseUnsignedManifestSchema,
	createSshRuntimeManifestSigningPayload,
	encodeSshRuntimeReleaseManifest,
	type SshRuntimeReleaseTrustDescriptor,
} from '@orchbee/ssh-environment-protocol';
import { describe, expect, it } from 'vitest';

import { verifySshRuntimeArtifact } from '../verify-ssh-runtime-artifact';

const darwinArtifact = Buffer.from('darwin runtime artifact');
const linuxArtifact = Buffer.from('linux runtime artifact');
const runtimeVersion = SshEnvironmentRuntimeVersionSchema.parse('0.1.0');

describe('SSH Runtime signed artifact verification', () => {
	it('admits bytes from the exact package-bound manifest and returns a detached artifact', () => {
		const release = createRelease();
		const verified = verifySshRuntimeArtifact({
			artifactBytes: darwinArtifact,
			expectedPlatform: 'darwin-arm64',
			manifestBytes: release.manifestBytes,
			trustDescriptor: release.trustDescriptor,
		});

		expect(verified).toEqual({
			kind: 'verified',
			artifact: {
				buildDigest: digest(darwinArtifact),
				bytes: Uint8Array.from(darwinArtifact),
				platform: 'darwin-arm64',
				runtimeVersion,
			},
		});
		if (verified.kind !== 'verified') throw new Error('expected verified artifact');
		expect(verified.artifact.bytes).not.toBe(darwinArtifact);
		const mutated = Buffer.from(darwinArtifact);
		mutated[0] = (mutated[0] ?? 0) ^ 1;
		expect(
			verifySshRuntimeArtifact({
				artifactBytes: mutated,
				expectedPlatform: 'darwin-arm64',
				manifestBytes: release.manifestBytes,
				trustDescriptor: release.trustDescriptor,
			}),
		).toEqual({ kind: 'failed', reason: 'artifact-digest-mismatch' });
	});

	it('rejects noncanonical manifest bytes and an unpinned valid manifest', () => {
		const release = createRelease();
		const noncanonical = Buffer.from(release.manifestBytes.subarray(0, -1));
		const unpinnedManifest = SshRuntimeReleaseManifestSchema.parse({
			...release.manifest,
			runtimeVersion: '0.1.1',
		});
		const unpinnedManifestBytes = encodeSshRuntimeReleaseManifest(unpinnedManifest);

		expect(
			verifySshRuntimeArtifact({
				artifactBytes: darwinArtifact,
				expectedPlatform: 'darwin-arm64',
				manifestBytes: noncanonical,
				trustDescriptor: release.trustDescriptor,
			}),
		).toEqual({ kind: 'failed', reason: 'manifest-invalid' });
		expect(
			verifySshRuntimeArtifact({
				artifactBytes: darwinArtifact,
				expectedPlatform: 'darwin-arm64',
				manifestBytes: unpinnedManifestBytes,
				trustDescriptor: release.trustDescriptor,
			}),
		).toEqual({ kind: 'failed', reason: 'manifest-trust-mismatch' });
	});

	it('rejects a substituted key, a mismatched key identity, and malformed trust', () => {
		const release = createRelease();
		const otherSigner = generateKeyPairSync('ed25519');
		const otherTrust = trustDescriptorFor(otherSigner.publicKey, release.manifestBytes);
		const mismatchedIdentity = {
			...release.trustDescriptor,
			signingKeyId: `${SSH_RUNTIME_RELEASE_SIGNING_KEY_ID_PREFIX}${'0'.repeat(64)}`,
		};

		expect(
			verifySshRuntimeArtifact({
				artifactBytes: darwinArtifact,
				expectedPlatform: 'darwin-arm64',
				manifestBytes: release.manifestBytes,
				trustDescriptor: otherTrust,
			}),
		).toEqual({ kind: 'failed', reason: 'manifest-signature-invalid' });
		expect(
			verifySshRuntimeArtifact({
				artifactBytes: darwinArtifact,
				expectedPlatform: 'darwin-arm64',
				manifestBytes: release.manifestBytes,
				trustDescriptor: mismatchedIdentity,
			}),
		).toEqual({ kind: 'failed', reason: 'trust-descriptor-invalid' });
		expect(
			verifySshRuntimeArtifact({
				artifactBytes: darwinArtifact,
				expectedPlatform: 'darwin-arm64',
				manifestBytes: release.manifestBytes,
				trustDescriptor: { ...release.trustDescriptor, publicKeySpki: 'invalid' },
			}),
		).toEqual({ kind: 'failed', reason: 'trust-descriptor-invalid' });
	});

	it('rejects size, platform-matrix, and signature mutations before admission', () => {
		const release = createRelease();
		const truncated = darwinArtifact.subarray(1);
		const invalidMatrixBytes = Buffer.from(
			`${JSON.stringify({
				...release.manifest,
				artifacts: [...release.manifest.artifacts].reverse(),
			})}\n`,
		);
		const invalidSignatureManifest = SshRuntimeReleaseManifestSchema.parse({
			...release.manifest,
			signature: `${release.manifest.signature.slice(0, -1)}${release.manifest.signature.endsWith('A') ? 'B' : 'A'}`,
		});
		const invalidSignatureBytes = encodeSshRuntimeReleaseManifest(invalidSignatureManifest);
		const invalidSignatureTrust = trustDescriptorFor(release.publicKey, invalidSignatureBytes);

		expect(
			verifySshRuntimeArtifact({
				artifactBytes: truncated,
				expectedPlatform: 'darwin-arm64',
				manifestBytes: release.manifestBytes,
				trustDescriptor: release.trustDescriptor,
			}),
		).toEqual({ kind: 'failed', reason: 'artifact-size-mismatch' });
		expect(
			verifySshRuntimeArtifact({
				artifactBytes: darwinArtifact,
				expectedPlatform: 'darwin-arm64',
				manifestBytes: invalidMatrixBytes,
				trustDescriptor: trustDescriptorFor(release.publicKey, invalidMatrixBytes),
			}),
		).toEqual({ kind: 'failed', reason: 'manifest-invalid' });
		expect(
			verifySshRuntimeArtifact({
				artifactBytes: darwinArtifact,
				expectedPlatform: 'darwin-arm64',
				manifestBytes: invalidSignatureBytes,
				trustDescriptor: invalidSignatureTrust,
			}),
		).toEqual({ kind: 'failed', reason: 'manifest-signature-invalid' });
	});
});

function createRelease() {
	const signer = generateKeyPairSync('ed25519');
	const unsignedManifest = SshRuntimeReleaseUnsignedManifestSchema.parse({
		schemaVersion: SSH_RUNTIME_RELEASE_SCHEMA_VERSION,
		runtimeVersion,
		artifacts: [
			{
				platform: 'darwin-arm64',
				sizeBytes: darwinArtifact.byteLength,
				sha256: digest(darwinArtifact),
			},
			{
				platform: 'linux-x64',
				sizeBytes: linuxArtifact.byteLength,
				sha256: digest(linuxArtifact),
			},
		],
	});
	const manifest = SshRuntimeReleaseManifestSchema.parse({
		...unsignedManifest,
		signature: sign(
			null,
			createSshRuntimeManifestSigningPayload(unsignedManifest),
			signer.privateKey,
		).toString('base64url'),
	});
	const manifestBytes = encodeSshRuntimeReleaseManifest(manifest);
	return {
		manifest,
		manifestBytes,
		publicKey: signer.publicKey,
		trustDescriptor: trustDescriptorFor(signer.publicKey, manifestBytes),
	};
}

function trustDescriptorFor(
	publicKey: KeyObject,
	manifestBytes: Uint8Array,
): SshRuntimeReleaseTrustDescriptor {
	const publicKeyBytes = publicKey.export({ format: 'der', type: 'spki' });
	return SshRuntimeReleaseTrustDescriptorSchema.parse({
		schemaVersion: SSH_RUNTIME_RELEASE_TRUST_SCHEMA_VERSION,
		signingKeyId: `${SSH_RUNTIME_RELEASE_SIGNING_KEY_ID_PREFIX}${createHash('sha256')
			.update(publicKeyBytes)
			.digest('hex')}`,
		publicKeySpki: publicKeyBytes.toString('base64url'),
		manifestSha256: digest(manifestBytes),
	});
}

function digest(bytes: Uint8Array) {
	return SshEnvironmentBuildDigestSchema.parse(createHash('sha256').update(bytes).digest('hex'));
}
