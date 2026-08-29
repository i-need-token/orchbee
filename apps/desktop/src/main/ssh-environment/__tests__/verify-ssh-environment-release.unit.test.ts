import { createHash, generateKeyPairSync, sign, type KeyObject } from 'node:crypto';

import {
	SSH_ENVIRONMENT_RELEASE_SCHEMA_VERSION,
	SSH_ENVIRONMENT_RELEASE_SIGNING_KEY_ID_PREFIX,
	SSH_ENVIRONMENT_RELEASE_TRUST_SCHEMA_VERSION,
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentReleaseManifestSchema,
	SshEnvironmentReleaseTrustDescriptorSchema,
	SshEnvironmentReleaseUnsignedManifestSchema,
	createSshEnvironmentReleaseManifestSigningPayload,
	encodeSshEnvironmentReleaseManifest,
	type SshEnvironmentReleaseTrustDescriptor,
} from '@orchbee/ssh-environment-protocol';
import { describe, expect, it } from 'vitest';

import {
	verifySshEnvironmentArtifactAgainstCatalog,
	verifySshEnvironmentReleaseCatalog,
} from '../verify-ssh-environment-release';

const bytes = {
	'ssh-environment-runtime': {
		'darwin-arm64': Buffer.from('darwin runtime artifact'),
		'linux-x64': Buffer.from('linux runtime artifact'),
	},
	'terminal-host': {
		'darwin-arm64': Buffer.from('darwin terminal host artifact'),
		'linux-x64': Buffer.from('linux terminal host artifact'),
	},
};

describe('SSH Environment signed release verification', () => {
	it('admits both exact components from one package-bound manifest', () => {
		const release = createRelease();
		const catalog = verifySshEnvironmentReleaseCatalog({
			manifestBytes: release.manifestBytes,
			trustDescriptor: release.trustDescriptor,
		});
		expect(catalog).toMatchObject({
			kind: 'verified',
			catalog: {
				components: {
					'ssh-environment-runtime': { role: 'ssh-environment-runtime', version: '1.2.3' },
					'terminal-host': { role: 'terminal-host', version: '4.5.6' },
				},
			},
		});

		const verified = verifySshEnvironmentArtifact({
			artifactBytes: bytes['terminal-host']['darwin-arm64'],
			expectedPlatform: 'darwin-arm64',
			expectedRole: 'terminal-host',
			manifestBytes: release.manifestBytes,
			trustDescriptor: release.trustDescriptor,
		});
		expect(verified).toEqual({
			artifact: {
				buildDigest: digest(bytes['terminal-host']['darwin-arm64']),
				bytes: Uint8Array.from(bytes['terminal-host']['darwin-arm64']),
				platform: 'darwin-arm64',
				role: 'terminal-host',
				version: '4.5.6',
			},
			kind: 'verified',
		});
	});

	it('rejects role substitution, changed bytes, and unpinned canonical manifests', () => {
		const release = createRelease();
		expect(
			verifySshEnvironmentArtifact({
				artifactBytes: bytes['terminal-host']['darwin-arm64'],
				expectedPlatform: 'darwin-arm64',
				expectedRole: 'ssh-environment-runtime',
				manifestBytes: release.manifestBytes,
				trustDescriptor: release.trustDescriptor,
			}),
		).toEqual({ kind: 'failed', reason: 'artifact-size-mismatch' });

		const changed = Buffer.from(bytes['terminal-host']['linux-x64']);
		changed[0] = (changed[0] ?? 0) ^ 1;
		expect(
			verifySshEnvironmentArtifact({
				artifactBytes: changed,
				expectedPlatform: 'linux-x64',
				expectedRole: 'terminal-host',
				manifestBytes: release.manifestBytes,
				trustDescriptor: release.trustDescriptor,
			}),
		).toEqual({ kind: 'failed', reason: 'artifact-digest-mismatch' });

		const unpinnedManifest = SshEnvironmentReleaseManifestSchema.parse({
			...release.manifest,
			components: [
				release.manifest.components[0],
				{ ...release.manifest.components[1], version: '4.5.7' },
			],
		});
		expect(
			verifySshEnvironmentReleaseCatalog({
				manifestBytes: encodeSshEnvironmentReleaseManifest(unpinnedManifest),
				trustDescriptor: release.trustDescriptor,
			}),
		).toEqual({ kind: 'failed', reason: 'manifest-trust-mismatch' });
	});

	it('rejects substituted keys, invalid signatures, and noncanonical manifests', () => {
		const release = createRelease();
		const otherSigner = generateKeyPairSync('ed25519');
		expect(
			verifySshEnvironmentReleaseCatalog({
				manifestBytes: release.manifestBytes,
				trustDescriptor: trustDescriptorFor(otherSigner.publicKey, release.manifestBytes),
			}),
		).toEqual({ kind: 'failed', reason: 'manifest-signature-invalid' });

		const invalidSignatureManifest = SshEnvironmentReleaseManifestSchema.parse({
			...release.manifest,
			signature: `${release.manifest.signature.slice(0, -1)}${release.manifest.signature.endsWith('A') ? 'B' : 'A'}`,
		});
		const invalidSignatureBytes = encodeSshEnvironmentReleaseManifest(invalidSignatureManifest);
		expect(
			verifySshEnvironmentReleaseCatalog({
				manifestBytes: invalidSignatureBytes,
				trustDescriptor: trustDescriptorFor(release.publicKey, invalidSignatureBytes),
			}),
		).toEqual({ kind: 'failed', reason: 'manifest-signature-invalid' });

		expect(
			verifySshEnvironmentReleaseCatalog({
				manifestBytes: release.manifestBytes.subarray(0, -1),
				trustDescriptor: release.trustDescriptor,
			}),
		).toEqual({ kind: 'failed', reason: 'manifest-invalid' });
	});
});

function verifySshEnvironmentArtifact(options: {
	readonly artifactBytes: Uint8Array;
	readonly expectedPlatform: 'darwin-arm64' | 'linux-x64';
	readonly expectedRole: 'ssh-environment-runtime' | 'terminal-host';
	readonly manifestBytes: Uint8Array;
	readonly trustDescriptor: unknown;
}) {
	const catalog = verifySshEnvironmentReleaseCatalog(options);
	return catalog.kind === 'failed'
		? catalog
		: verifySshEnvironmentArtifactAgainstCatalog({
				artifactBytes: options.artifactBytes,
				catalog: catalog.catalog,
				expectedPlatform: options.expectedPlatform,
				expectedRole: options.expectedRole,
			});
}

function createRelease() {
	const signer = generateKeyPairSync('ed25519');
	const unsignedManifest = SshEnvironmentReleaseUnsignedManifestSchema.parse({
		schemaVersion: SSH_ENVIRONMENT_RELEASE_SCHEMA_VERSION,
		components: [
			{
				role: 'ssh-environment-runtime',
				version: '1.2.3',
				artifacts: createArtifactRecords(bytes['ssh-environment-runtime']),
			},
			{
				role: 'terminal-host',
				version: '4.5.6',
				artifacts: createArtifactRecords(bytes['terminal-host']),
			},
		],
	});
	const manifest = SshEnvironmentReleaseManifestSchema.parse({
		...unsignedManifest,
		signature: sign(
			null,
			createSshEnvironmentReleaseManifestSigningPayload(unsignedManifest),
			signer.privateKey,
		).toString('base64url'),
	});
	const manifestBytes = encodeSshEnvironmentReleaseManifest(manifest);
	return {
		manifest,
		manifestBytes,
		publicKey: signer.publicKey,
		trustDescriptor: trustDescriptorFor(signer.publicKey, manifestBytes),
	};
}

function createArtifactRecords(componentBytes: {
	readonly 'darwin-arm64': Uint8Array;
	readonly 'linux-x64': Uint8Array;
}) {
	return [
		{
			platform: 'darwin-arm64',
			sha256: digest(componentBytes['darwin-arm64']),
			sizeBytes: componentBytes['darwin-arm64'].byteLength,
		},
		{
			platform: 'linux-x64',
			sha256: digest(componentBytes['linux-x64']),
			sizeBytes: componentBytes['linux-x64'].byteLength,
		},
	];
}

function trustDescriptorFor(
	publicKey: KeyObject,
	manifestBytes: Uint8Array,
): SshEnvironmentReleaseTrustDescriptor {
	const publicKeyBytes = publicKey.export({ format: 'der', type: 'spki' });
	return SshEnvironmentReleaseTrustDescriptorSchema.parse({
		schemaVersion: SSH_ENVIRONMENT_RELEASE_TRUST_SCHEMA_VERSION,
		signingKeyId: `${SSH_ENVIRONMENT_RELEASE_SIGNING_KEY_ID_PREFIX}${createHash('sha256')
			.update(publicKeyBytes)
			.digest('hex')}`,
		publicKeySpki: publicKeyBytes.toString('base64url'),
		manifestSha256: digest(manifestBytes),
	});
}

function digest(value: Uint8Array) {
	return SshEnvironmentBuildDigestSchema.parse(createHash('sha256').update(value).digest('hex'));
}
