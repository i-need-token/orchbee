import { z } from 'zod';

import {
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentPlatformSchema,
	SshEnvironmentRuntimeVersionSchema,
	type SshEnvironmentPlatform,
} from './ssh-environment-protocol';

export const SSH_RUNTIME_RELEASE_SCHEMA_VERSION = 1;
export const SSH_RUNTIME_RELEASE_TRUST_SCHEMA_VERSION = 1;
export const SSH_RUNTIME_RELEASE_MAXIMUM_ARTIFACT_BYTES = 128 * 1024 * 1024;
export const SSH_RUNTIME_RELEASE_MAXIMUM_MANIFEST_BYTES = 64 * 1024;
export const SSH_RUNTIME_RELEASE_DIRECTORY_NAME = 'ssh-environment-runtime-release-v1';
export const SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME = 'manifest.json';
export const SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME = 'manifest-signing-payload.json';
export const SSH_RUNTIME_RELEASE_TRUST_DESCRIPTOR_FILE_NAME =
	'ssh-environment-runtime-release-trust-v1.json';
export const SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME = 'orchbee-ssh-environment-runtime';
export const SSH_RUNTIME_RELEASE_SIGNING_KEY_ID_PREFIX = 'ed25519-spki-sha256:';
export const SSH_RUNTIME_RELEASE_PLATFORMS = Object.freeze([
	'darwin-arm64',
	'linux-x64',
] as const satisfies readonly SshEnvironmentPlatform[]);
export const SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS = Object.freeze({
	'darwin-arm64': `artifacts/darwin-arm64/${SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME}`,
	'linux-x64': `artifacts/linux-x64/${SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME}`,
} satisfies Readonly<Record<SshEnvironmentPlatform, string>>);

const textEncoder = new TextEncoder();
const Ed25519SignatureSchema = z
	.string()
	.length(86)
	.regex(/^[A-Za-z0-9_-]+$/u);
const Ed25519SpkiSchema = z
	.string()
	.length(59)
	.regex(/^[A-Za-z0-9_-]+$/u);
const Ed25519SigningKeyIdSchema = z.string().regex(/^ed25519-spki-sha256:[0-9a-f]{64}$/u);

export const SshRuntimeReleaseArtifactSchema = z.strictObject({
	platform: SshEnvironmentPlatformSchema,
	sizeBytes: z.number().int().min(1).max(SSH_RUNTIME_RELEASE_MAXIMUM_ARTIFACT_BYTES),
	sha256: SshEnvironmentBuildDigestSchema,
});

export const SshRuntimeReleaseUnsignedManifestSchema = z
	.strictObject({
		schemaVersion: z.literal(SSH_RUNTIME_RELEASE_SCHEMA_VERSION),
		runtimeVersion: SshEnvironmentRuntimeVersionSchema,
		artifacts: z
			.array(SshRuntimeReleaseArtifactSchema)
			.length(SSH_RUNTIME_RELEASE_PLATFORMS.length),
	})
	.superRefine(assertCanonicalPlatformMatrix);

export const SshRuntimeReleaseManifestSchema = z
	.strictObject({
		schemaVersion: z.literal(SSH_RUNTIME_RELEASE_SCHEMA_VERSION),
		runtimeVersion: SshEnvironmentRuntimeVersionSchema,
		artifacts: z
			.array(SshRuntimeReleaseArtifactSchema)
			.length(SSH_RUNTIME_RELEASE_PLATFORMS.length),
		signature: Ed25519SignatureSchema,
	})
	.superRefine(assertCanonicalPlatformMatrix);

export const SshRuntimeReleaseTrustDescriptorSchema = z.strictObject({
	schemaVersion: z.literal(SSH_RUNTIME_RELEASE_TRUST_SCHEMA_VERSION),
	signingKeyId: Ed25519SigningKeyIdSchema,
	publicKeySpki: Ed25519SpkiSchema,
	manifestSha256: SshEnvironmentBuildDigestSchema,
});

export type SshRuntimeReleaseArtifact = z.infer<typeof SshRuntimeReleaseArtifactSchema>;
export type SshRuntimeReleaseUnsignedManifest = z.infer<
	typeof SshRuntimeReleaseUnsignedManifestSchema
>;
export type SshRuntimeReleaseManifest = z.infer<typeof SshRuntimeReleaseManifestSchema>;
export type SshRuntimeReleaseTrustDescriptor = z.infer<
	typeof SshRuntimeReleaseTrustDescriptorSchema
>;

export function createSshRuntimeManifestSigningPayload(
	manifest: SshRuntimeReleaseUnsignedManifest,
): Uint8Array {
	const canonical = SshRuntimeReleaseUnsignedManifestSchema.parse(manifest);
	return textEncoder.encode(JSON.stringify(projectUnsignedManifest(canonical)));
}

export function encodeSshRuntimeReleaseManifest(manifest: SshRuntimeReleaseManifest): Uint8Array {
	const canonical = SshRuntimeReleaseManifestSchema.parse(manifest);
	return textEncoder.encode(
		`${JSON.stringify({ ...projectUnsignedManifest(canonical), signature: canonical.signature })}\n`,
	);
}

export function encodeSshRuntimeReleaseTrustDescriptor(
	descriptor: SshRuntimeReleaseTrustDescriptor,
): Uint8Array {
	const canonical = SshRuntimeReleaseTrustDescriptorSchema.parse(descriptor);
	return textEncoder.encode(
		`${JSON.stringify({
			schemaVersion: canonical.schemaVersion,
			signingKeyId: canonical.signingKeyId,
			publicKeySpki: canonical.publicKeySpki,
			manifestSha256: canonical.manifestSha256,
		})}\n`,
	);
}

function assertCanonicalPlatformMatrix(
	manifest: { readonly artifacts: readonly SshRuntimeReleaseArtifact[] },
	context: z.RefinementCtx,
): void {
	for (const [index, expectedPlatform] of SSH_RUNTIME_RELEASE_PLATFORMS.entries()) {
		if (manifest.artifacts[index]?.platform === expectedPlatform) continue;
		context.addIssue({
			code: 'custom',
			message: 'Runtime artifacts must contain the canonical supported platform matrix.',
			path: ['artifacts', index, 'platform'],
		});
	}
}

function projectUnsignedManifest(manifest: SshRuntimeReleaseUnsignedManifest) {
	return {
		schemaVersion: manifest.schemaVersion,
		runtimeVersion: manifest.runtimeVersion,
		artifacts: manifest.artifacts.map((artifact) => ({
			platform: artifact.platform,
			sizeBytes: artifact.sizeBytes,
			sha256: artifact.sha256,
		})),
	};
}
