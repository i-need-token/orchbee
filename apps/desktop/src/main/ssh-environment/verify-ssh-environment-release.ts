import { createHash, createPublicKey, verify } from 'node:crypto';

import {
	SSH_ENVIRONMENT_RELEASE_MAXIMUM_MANIFEST_BYTES,
	SSH_ENVIRONMENT_RELEASE_SIGNING_KEY_ID_PREFIX,
	SshEnvironmentReleaseManifestSchema,
	SshEnvironmentReleaseCohortDigestSchema,
	SshEnvironmentReleaseTrustDescriptorSchema,
	createSshEnvironmentReleaseManifestSigningPayload,
	encodeSshEnvironmentReleaseManifest,
	type SshEnvironmentBuildDigest,
	type SshEnvironmentPlatform,
	type SshEnvironmentReleaseComponentRole,
	type SshEnvironmentReleaseCohortDigest,
	type SshEnvironmentReleaseManifest,
	type SshEnvironmentRuntimeVersion,
	type SshEnvironmentTerminalHostVersion,
} from '@orchbee/ssh-environment-protocol';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

interface VerifiedSshEnvironmentArtifactDescriptor {
	readonly buildDigest: SshEnvironmentBuildDigest;
	readonly platform: SshEnvironmentPlatform;
	readonly sizeBytes: number;
}

interface VerifiedSshEnvironmentRuntimeComponentCatalog {
	readonly artifacts: Readonly<
		Record<SshEnvironmentPlatform, VerifiedSshEnvironmentArtifactDescriptor>
	>;
	readonly role: 'ssh-environment-runtime';
	readonly version: SshEnvironmentRuntimeVersion;
}

interface VerifiedSshEnvironmentTerminalHostComponentCatalog {
	readonly artifacts: Readonly<
		Record<SshEnvironmentPlatform, VerifiedSshEnvironmentArtifactDescriptor>
	>;
	readonly role: 'terminal-host';
	readonly version: SshEnvironmentTerminalHostVersion;
}

export interface VerifiedSshEnvironmentReleaseCatalog {
	readonly components: Readonly<{
		readonly 'ssh-environment-runtime': VerifiedSshEnvironmentRuntimeComponentCatalog;
		readonly 'terminal-host': VerifiedSshEnvironmentTerminalHostComponentCatalog;
	}>;
	readonly releaseCohortDigest: SshEnvironmentReleaseCohortDigest;
}

export interface VerifiedSshEnvironmentArtifact {
	readonly buildDigest: SshEnvironmentBuildDigest;
	readonly bytes: Uint8Array;
	readonly platform: SshEnvironmentPlatform;
	readonly role: SshEnvironmentReleaseComponentRole;
	readonly version: SshEnvironmentRuntimeVersion | SshEnvironmentTerminalHostVersion;
}

type SshEnvironmentReleaseCatalogVerificationFailureReason =
	| 'manifest-invalid'
	| 'manifest-signature-invalid'
	| 'manifest-trust-mismatch'
	| 'trust-descriptor-invalid';

export type SshEnvironmentReleaseCatalogVerificationResult =
	| { readonly catalog: VerifiedSshEnvironmentReleaseCatalog; readonly kind: 'verified' }
	| {
			readonly kind: 'failed';
			readonly reason: SshEnvironmentReleaseCatalogVerificationFailureReason;
	  };

export type SshEnvironmentArtifactVerificationResult =
	| { readonly artifact: VerifiedSshEnvironmentArtifact; readonly kind: 'verified' }
	| {
			readonly kind: 'failed';
			readonly reason: 'artifact-digest-mismatch' | 'artifact-size-mismatch';
	  };

export function verifySshEnvironmentReleaseCatalog(options: {
	readonly manifestBytes: Uint8Array;
	readonly trustDescriptor: unknown;
}): SshEnvironmentReleaseCatalogVerificationResult {
	const trust = SshEnvironmentReleaseTrustDescriptorSchema.safeParse(options.trustDescriptor);
	if (!trust.success) return { kind: 'failed', reason: 'trust-descriptor-invalid' };

	const publicKey = resolveTrustedPublicKey(trust.data.publicKeySpki, trust.data.signingKeyId);
	if (publicKey === null) return { kind: 'failed', reason: 'trust-descriptor-invalid' };

	const manifest = decodeCanonicalManifest(options.manifestBytes);
	if (manifest === null) return { kind: 'failed', reason: 'manifest-invalid' };

	const manifestDigest = SshEnvironmentReleaseCohortDigestSchema.parse(
		createHash('sha256').update(options.manifestBytes).digest('hex'),
	);
	if (manifestDigest !== trust.data.manifestSha256) {
		return { kind: 'failed', reason: 'manifest-trust-mismatch' };
	}
	if (!verifyManifestSignature(manifest, publicKey)) {
		return { kind: 'failed', reason: 'manifest-signature-invalid' };
	}

	const runtime = manifest.components[0];
	const terminalHost = manifest.components[1];
	return {
		catalog: Object.freeze({
			components: Object.freeze({
				'ssh-environment-runtime': Object.freeze({
					artifacts: createArtifactDescriptors(runtime.artifacts),
					role: runtime.role,
					version: runtime.version,
				}),
				'terminal-host': Object.freeze({
					artifacts: createArtifactDescriptors(terminalHost.artifacts),
					role: terminalHost.role,
					version: terminalHost.version,
				}),
			}),
			releaseCohortDigest: manifestDigest,
		}),
		kind: 'verified',
	};
}

export function verifySshEnvironmentArtifactAgainstCatalog(options: {
	readonly artifactBytes: Uint8Array;
	readonly catalog: VerifiedSshEnvironmentReleaseCatalog;
	readonly expectedPlatform: SshEnvironmentPlatform;
	readonly expectedRole: SshEnvironmentReleaseComponentRole;
}): SshEnvironmentArtifactVerificationResult {
	const component = options.catalog.components[options.expectedRole];
	const descriptor = component.artifacts[options.expectedPlatform];
	if (options.artifactBytes.byteLength !== descriptor.sizeBytes) {
		return { kind: 'failed', reason: 'artifact-size-mismatch' };
	}
	const actualDigest = createHash('sha256').update(options.artifactBytes).digest('hex');
	if (actualDigest !== descriptor.buildDigest) {
		return { kind: 'failed', reason: 'artifact-digest-mismatch' };
	}
	return {
		artifact: {
			buildDigest: descriptor.buildDigest,
			bytes: Uint8Array.from(options.artifactBytes),
			platform: descriptor.platform,
			role: component.role,
			version: component.version,
		},
		kind: 'verified',
	};
}

function createArtifactDescriptors(
	artifacts: SshEnvironmentReleaseManifest['components'][number]['artifacts'],
): Readonly<Record<SshEnvironmentPlatform, VerifiedSshEnvironmentArtifactDescriptor>> {
	return Object.freeze({
		'darwin-arm64': Object.freeze({
			buildDigest: artifacts[0].sha256,
			platform: artifacts[0].platform,
			sizeBytes: artifacts[0].sizeBytes,
		}),
		'linux-x64': Object.freeze({
			buildDigest: artifacts[1].sha256,
			platform: artifacts[1].platform,
			sizeBytes: artifacts[1].sizeBytes,
		}),
	});
}

function verifyManifestSignature(
	manifest: SshEnvironmentReleaseManifest,
	publicKey: ReturnType<typeof createPublicKey>,
): boolean {
	const { signature, ...unsignedManifest } = manifest;
	const signatureBytes = Buffer.from(signature, 'base64url');
	try {
		return (
			signatureBytes.byteLength === 64 &&
			signatureBytes.toString('base64url') === signature &&
			verify(
				null,
				createSshEnvironmentReleaseManifestSigningPayload(unsignedManifest),
				publicKey,
				signatureBytes,
			)
		);
	} catch {
		return false;
	}
}

function decodeCanonicalManifest(manifestBytes: Uint8Array): SshEnvironmentReleaseManifest | null {
	if (
		manifestBytes.byteLength === 0 ||
		manifestBytes.byteLength > SSH_ENVIRONMENT_RELEASE_MAXIMUM_MANIFEST_BYTES
	) {
		return null;
	}
	let value: unknown;
	try {
		value = JSON.parse(utf8Decoder.decode(manifestBytes));
	} catch {
		return null;
	}
	const parsed = SshEnvironmentReleaseManifestSchema.safeParse(value);
	if (!parsed.success) return null;
	return Buffer.from(encodeSshEnvironmentReleaseManifest(parsed.data)).equals(
		Buffer.from(manifestBytes),
	)
		? parsed.data
		: null;
}

function resolveTrustedPublicKey(publicKeySpki: string, expectedSigningKeyId: string) {
	const encoded = Buffer.from(publicKeySpki, 'base64url');
	if (encoded.toString('base64url') !== publicKeySpki) return null;
	try {
		const publicKey = createPublicKey({ format: 'der', key: encoded, type: 'spki' });
		if (publicKey.asymmetricKeyType !== 'ed25519') return null;
		const canonical = publicKey.export({ format: 'der', type: 'spki' });
		if (!Buffer.isBuffer(canonical) || !canonical.equals(encoded)) return null;
		const signingKeyId = `${SSH_ENVIRONMENT_RELEASE_SIGNING_KEY_ID_PREFIX}${createHash('sha256')
			.update(canonical)
			.digest('hex')}`;
		return signingKeyId === expectedSigningKeyId ? publicKey : null;
	} catch {
		return null;
	}
}
