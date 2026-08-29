import { createHash, createPublicKey, verify } from 'node:crypto';

import {
	SSH_RUNTIME_RELEASE_MAXIMUM_MANIFEST_BYTES,
	SSH_RUNTIME_RELEASE_SIGNING_KEY_ID_PREFIX,
	SshRuntimeReleaseManifestSchema,
	SshRuntimeReleaseTrustDescriptorSchema,
	createSshRuntimeManifestSigningPayload,
	encodeSshRuntimeReleaseManifest,
	type SshEnvironmentBuildDigest,
	type SshEnvironmentPlatform,
	type SshEnvironmentRuntimeVersion,
	type SshRuntimeReleaseManifest,
} from '@orchbee/ssh-environment-protocol';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export interface VerifiedSshRuntimeArtifactDescriptor {
	readonly buildDigest: SshEnvironmentBuildDigest;
	readonly platform: SshEnvironmentPlatform;
	readonly runtimeVersion: SshEnvironmentRuntimeVersion;
	readonly sizeBytes: number;
}

export interface VerifiedSshRuntimeArtifact {
	readonly bytes: Uint8Array;
	readonly buildDigest: SshEnvironmentBuildDigest;
	readonly platform: SshEnvironmentPlatform;
	readonly runtimeVersion: SshEnvironmentRuntimeVersion;
}

export interface VerifiedSshRuntimeReleaseCatalog {
	readonly artifacts: Readonly<
		Record<SshEnvironmentPlatform, VerifiedSshRuntimeArtifactDescriptor>
	>;
	readonly runtimeVersion: SshEnvironmentRuntimeVersion;
}

type SshRuntimeReleaseCatalogVerificationFailureReason =
	| 'manifest-invalid'
	| 'manifest-signature-invalid'
	| 'manifest-trust-mismatch'
	| 'trust-descriptor-invalid';

export type SshRuntimeReleaseCatalogVerificationResult =
	| { readonly catalog: VerifiedSshRuntimeReleaseCatalog; readonly kind: 'verified' }
	| {
			readonly kind: 'failed';
			readonly reason: SshRuntimeReleaseCatalogVerificationFailureReason;
	  };

export type SshRuntimeArtifactVerificationResult =
	| { readonly kind: 'verified'; readonly artifact: VerifiedSshRuntimeArtifact }
	| {
			readonly kind: 'failed';
			readonly reason: 'artifact-digest-mismatch' | 'artifact-size-mismatch';
	  };

export type SshRuntimeReleaseVerificationResult =
	| { readonly kind: 'verified'; readonly artifact: VerifiedSshRuntimeArtifact }
	| {
			readonly kind: 'failed';
			readonly reason:
				| SshRuntimeReleaseCatalogVerificationFailureReason
				| 'artifact-digest-mismatch'
				| 'artifact-size-mismatch';
	  };

export function verifySshRuntimeReleaseCatalog(options: {
	readonly manifestBytes: Uint8Array;
	readonly trustDescriptor: unknown;
}): SshRuntimeReleaseCatalogVerificationResult {
	const trust = SshRuntimeReleaseTrustDescriptorSchema.safeParse(options.trustDescriptor);
	if (!trust.success) return { kind: 'failed', reason: 'trust-descriptor-invalid' };

	const publicKeyResult = resolveTrustedPublicKey(
		trust.data.publicKeySpki,
		trust.data.signingKeyId,
	);
	if (publicKeyResult === null) return { kind: 'failed', reason: 'trust-descriptor-invalid' };

	const manifest = decodeCanonicalManifest(options.manifestBytes);
	if (manifest === null) return { kind: 'failed', reason: 'manifest-invalid' };

	const manifestDigest = createHash('sha256').update(options.manifestBytes).digest('hex');
	if (manifestDigest !== trust.data.manifestSha256) {
		return { kind: 'failed', reason: 'manifest-trust-mismatch' };
	}

	if (!verifyManifestSignature(manifest, publicKeyResult)) {
		return { kind: 'failed', reason: 'manifest-signature-invalid' };
	}

	const darwinArtifact = manifest.artifacts.find(
		(artifact) => artifact.platform === 'darwin-arm64',
	);
	const linuxArtifact = manifest.artifacts.find((artifact) => artifact.platform === 'linux-x64');
	if (darwinArtifact === undefined || linuxArtifact === undefined) {
		return { kind: 'failed', reason: 'manifest-invalid' };
	}
	const descriptors: Readonly<
		Record<SshEnvironmentPlatform, VerifiedSshRuntimeArtifactDescriptor>
	> = Object.freeze({
		'darwin-arm64': Object.freeze({
			buildDigest: darwinArtifact.sha256,
			platform: darwinArtifact.platform,
			runtimeVersion: manifest.runtimeVersion,
			sizeBytes: darwinArtifact.sizeBytes,
		}),
		'linux-x64': Object.freeze({
			buildDigest: linuxArtifact.sha256,
			platform: linuxArtifact.platform,
			runtimeVersion: manifest.runtimeVersion,
			sizeBytes: linuxArtifact.sizeBytes,
		}),
	});
	return {
		catalog: Object.freeze({
			artifacts: Object.freeze(descriptors),
			runtimeVersion: manifest.runtimeVersion,
		}),
		kind: 'verified',
	};
}

export function verifySshRuntimeArtifactAgainstCatalog(options: {
	readonly artifactBytes: Uint8Array;
	readonly catalog: VerifiedSshRuntimeReleaseCatalog;
	readonly expectedPlatform: SshEnvironmentPlatform;
}): SshRuntimeArtifactVerificationResult {
	const descriptor = options.catalog.artifacts[options.expectedPlatform];
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
			runtimeVersion: descriptor.runtimeVersion,
		},
		kind: 'verified',
	};
}

export function verifySshRuntimeArtifact(options: {
	readonly artifactBytes: Uint8Array;
	readonly expectedPlatform: SshEnvironmentPlatform;
	readonly manifestBytes: Uint8Array;
	readonly trustDescriptor: unknown;
}): SshRuntimeReleaseVerificationResult {
	const catalog = verifySshRuntimeReleaseCatalog(options);
	if (catalog.kind === 'failed') return catalog;
	return verifySshRuntimeArtifactAgainstCatalog({
		artifactBytes: options.artifactBytes,
		catalog: catalog.catalog,
		expectedPlatform: options.expectedPlatform,
	});
}

function verifyManifestSignature(
	manifest: SshRuntimeReleaseManifest,
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
				createSshRuntimeManifestSigningPayload(unsignedManifest),
				publicKey,
				signatureBytes,
			)
		);
	} catch {
		return false;
	}
}

function decodeCanonicalManifest(manifestBytes: Uint8Array): SshRuntimeReleaseManifest | null {
	if (
		manifestBytes.byteLength === 0 ||
		manifestBytes.byteLength > SSH_RUNTIME_RELEASE_MAXIMUM_MANIFEST_BYTES
	) {
		return null;
	}
	let value: unknown;
	try {
		value = JSON.parse(utf8Decoder.decode(manifestBytes));
	} catch {
		return null;
	}
	const parsed = SshRuntimeReleaseManifestSchema.safeParse(value);
	if (!parsed.success) return null;
	return Buffer.from(encodeSshRuntimeReleaseManifest(parsed.data)).equals(
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
		const signingKeyId = `${SSH_RUNTIME_RELEASE_SIGNING_KEY_ID_PREFIX}${createHash('sha256')
			.update(canonical)
			.digest('hex')}`;
		return signingKeyId === expectedSigningKeyId ? publicKey : null;
	} catch {
		return null;
	}
}
