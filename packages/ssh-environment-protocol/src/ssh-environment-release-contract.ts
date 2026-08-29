import { z } from 'zod';

import {
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentPlatformSchema,
	SshEnvironmentReleaseCohortDigestSchema,
	SshEnvironmentRuntimeVersionSchema,
	SshEnvironmentTerminalHostVersionSchema,
	type SshEnvironmentPlatform,
} from './ssh-environment-protocol';

export const SSH_ENVIRONMENT_RELEASE_SCHEMA_VERSION = 2;
export const SSH_ENVIRONMENT_RELEASE_TRUST_SCHEMA_VERSION = 2;
export const SSH_ENVIRONMENT_RELEASE_MAXIMUM_ARTIFACT_BYTES = 128 * 1024 * 1024;
export const SSH_ENVIRONMENT_RELEASE_MAXIMUM_MANIFEST_BYTES = 64 * 1024;
export const SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME = 'ssh-environment-release-v2';
export const SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME = 'manifest.json';
export const SSH_ENVIRONMENT_RELEASE_SIGNING_PAYLOAD_FILE_NAME = 'manifest-signing-payload.json';
export const SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME =
	'ssh-environment-release-trust-v2.json';
export const SSH_ENVIRONMENT_RELEASE_SIGNING_KEY_ID_PREFIX = 'ed25519-spki-sha256:';
export const SSH_ENVIRONMENT_RELEASE_PLATFORMS = Object.freeze([
	'darwin-arm64',
	'linux-x64',
] as const satisfies readonly SshEnvironmentPlatform[]);
export const SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES = Object.freeze([
	'ssh-environment-runtime',
	'terminal-host',
] as const);
export const SSH_ENVIRONMENT_RELEASE_ARTIFACT_FILE_NAMES = Object.freeze({
	'ssh-environment-runtime': 'orchbee-ssh-environment-runtime',
	'terminal-host': 'orchbee-terminal-host',
} as const);
export const SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS = Object.freeze({
	'ssh-environment-runtime': Object.freeze({
		'darwin-arm64': 'artifacts/darwin-arm64/orchbee-ssh-environment-runtime',
		'linux-x64': 'artifacts/linux-x64/orchbee-ssh-environment-runtime',
	}),
	'terminal-host': Object.freeze({
		'darwin-arm64': 'artifacts/darwin-arm64/orchbee-terminal-host',
		'linux-x64': 'artifacts/linux-x64/orchbee-terminal-host',
	}),
} as const);

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
export const SshEnvironmentReleaseArtifactSchema = z.strictObject({
	platform: SshEnvironmentPlatformSchema,
	sizeBytes: z.number().int().min(1).max(SSH_ENVIRONMENT_RELEASE_MAXIMUM_ARTIFACT_BYTES),
	sha256: SshEnvironmentBuildDigestSchema,
});

const SshEnvironmentReleaseDarwinArtifactSchema = SshEnvironmentReleaseArtifactSchema.extend({
	platform: z.literal('darwin-arm64'),
});
const SshEnvironmentReleaseLinuxArtifactSchema = SshEnvironmentReleaseArtifactSchema.extend({
	platform: z.literal('linux-x64'),
});
const SshEnvironmentReleaseArtifactMatrixSchema = z.tuple([
	SshEnvironmentReleaseDarwinArtifactSchema,
	SshEnvironmentReleaseLinuxArtifactSchema,
]);

export const SshEnvironmentReleaseRuntimeComponentSchema = z.strictObject({
	role: z.literal('ssh-environment-runtime'),
	version: SshEnvironmentRuntimeVersionSchema,
	artifacts: SshEnvironmentReleaseArtifactMatrixSchema,
});
export const SshEnvironmentReleaseTerminalHostComponentSchema = z.strictObject({
	role: z.literal('terminal-host'),
	version: SshEnvironmentTerminalHostVersionSchema,
	artifacts: SshEnvironmentReleaseArtifactMatrixSchema,
});

export const SshEnvironmentReleaseUnsignedManifestSchema = z.strictObject({
	schemaVersion: z.literal(SSH_ENVIRONMENT_RELEASE_SCHEMA_VERSION),
	components: z.tuple([
		SshEnvironmentReleaseRuntimeComponentSchema,
		SshEnvironmentReleaseTerminalHostComponentSchema,
	]),
});

export const SshEnvironmentReleaseManifestSchema = z.strictObject({
	schemaVersion: z.literal(SSH_ENVIRONMENT_RELEASE_SCHEMA_VERSION),
	components: z.tuple([
		SshEnvironmentReleaseRuntimeComponentSchema,
		SshEnvironmentReleaseTerminalHostComponentSchema,
	]),
	signature: Ed25519SignatureSchema,
});

export const SshEnvironmentReleaseTrustDescriptorSchema = z.strictObject({
	schemaVersion: z.literal(SSH_ENVIRONMENT_RELEASE_TRUST_SCHEMA_VERSION),
	signingKeyId: Ed25519SigningKeyIdSchema,
	publicKeySpki: Ed25519SpkiSchema,
	manifestSha256: SshEnvironmentReleaseCohortDigestSchema,
});
export type SshEnvironmentReleaseArtifact = z.infer<typeof SshEnvironmentReleaseArtifactSchema>;
export type SshEnvironmentReleaseRuntimeComponent = z.infer<
	typeof SshEnvironmentReleaseRuntimeComponentSchema
>;
export type SshEnvironmentReleaseTerminalHostComponent = z.infer<
	typeof SshEnvironmentReleaseTerminalHostComponentSchema
>;
export type SshEnvironmentReleaseComponent =
	| SshEnvironmentReleaseRuntimeComponent
	| SshEnvironmentReleaseTerminalHostComponent;
export type SshEnvironmentReleaseComponentRole =
	(typeof SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES)[number];
export type SshEnvironmentReleaseUnsignedManifest = z.infer<
	typeof SshEnvironmentReleaseUnsignedManifestSchema
>;
export type SshEnvironmentReleaseManifest = z.infer<typeof SshEnvironmentReleaseManifestSchema>;
export type SshEnvironmentReleaseTrustDescriptor = z.infer<
	typeof SshEnvironmentReleaseTrustDescriptorSchema
>;

export function createSshEnvironmentReleaseManifestSigningPayload(
	manifest: SshEnvironmentReleaseUnsignedManifest,
): Uint8Array {
	const canonical = SshEnvironmentReleaseUnsignedManifestSchema.parse(manifest);
	return textEncoder.encode(JSON.stringify(projectUnsignedManifest(canonical)));
}

export function encodeSshEnvironmentReleaseManifest(
	manifest: SshEnvironmentReleaseManifest,
): Uint8Array {
	const canonical = SshEnvironmentReleaseManifestSchema.parse(manifest);
	return textEncoder.encode(
		`${JSON.stringify({ ...projectUnsignedManifest(canonical), signature: canonical.signature })}\n`,
	);
}

export function encodeSshEnvironmentReleaseTrustDescriptor(
	descriptor: SshEnvironmentReleaseTrustDescriptor,
): Uint8Array {
	const canonical = SshEnvironmentReleaseTrustDescriptorSchema.parse(descriptor);
	return textEncoder.encode(
		`${JSON.stringify({
			schemaVersion: canonical.schemaVersion,
			signingKeyId: canonical.signingKeyId,
			publicKeySpki: canonical.publicKeySpki,
			manifestSha256: canonical.manifestSha256,
		})}\n`,
	);
}

function projectUnsignedManifest(manifest: SshEnvironmentReleaseUnsignedManifest) {
	return {
		schemaVersion: manifest.schemaVersion,
		components: manifest.components.map((component) => ({
			role: component.role,
			version: component.version,
			artifacts: component.artifacts.map((artifact) => ({
				platform: artifact.platform,
				sizeBytes: artifact.sizeBytes,
				sha256: artifact.sha256,
			})),
		})),
	};
}
