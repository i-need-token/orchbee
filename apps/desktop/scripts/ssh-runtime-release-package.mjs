import { createHash, createPublicKey } from 'node:crypto';
import { lstat, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

import {
	SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME,
	SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_RUNTIME_RELEASE_DIRECTORY_NAME,
	SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME,
	SSH_RUNTIME_RELEASE_MAXIMUM_ARTIFACT_BYTES,
	SSH_RUNTIME_RELEASE_MAXIMUM_MANIFEST_BYTES,
	SSH_RUNTIME_RELEASE_PLATFORMS,
	SSH_RUNTIME_RELEASE_SIGNING_KEY_ID_PREFIX,
	SSH_RUNTIME_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
	SSH_RUNTIME_RELEASE_TRUST_SCHEMA_VERSION,
	SshRuntimeReleaseTrustDescriptorSchema,
	encodeSshRuntimeReleaseTrustDescriptor,
} from '@orchbee/ssh-environment-protocol';

import { verifySshRuntimeArtifact } from '../src/main/ssh-environment/verify-ssh-runtime-artifact.ts';

const MAXIMUM_PUBLIC_KEY_BYTES = 16 * 1024;

export function resolveSshRuntimeReleasePackageConfiguration(arguments_, environment) {
	if (arguments_.length === 0) return null;
	if (arguments_.length !== 1 || arguments_[0] !== '--with-ssh-runtime-release') {
		throw new Error('Desktop packaging received an unsupported argument.');
	}
	return {
		publicKeyFilePath: requireAbsolutePath(
			environment.ORCHBEE_SSH_RUNTIME_RELEASE_PUBLIC_KEY,
			'ORCHBEE_SSH_RUNTIME_RELEASE_PUBLIC_KEY',
		),
		releaseDirectory: requireAbsolutePath(
			environment.ORCHBEE_SSH_RUNTIME_RELEASE_DIRECTORY,
			'ORCHBEE_SSH_RUNTIME_RELEASE_DIRECTORY',
		),
	};
}

export async function stageSshRuntimeReleasePackageResources(options) {
	const applicationStagingRoot = requireAbsolutePath(
		options.applicationStagingRoot,
		'Application staging root',
	);
	const externalResourcesStagingRoot = requireAbsolutePath(
		options.externalResourcesStagingRoot,
		'External Resources staging root',
	);
	const verifiedRelease = await readVerifiedSshRuntimeRelease({
		publicKeyFilePath: options.publicKeyFilePath,
		releaseDirectory: options.releaseDirectory,
	});
	const trustDescriptorFilePath = path.join(
		applicationStagingRoot,
		'dist',
		'main',
		SSH_RUNTIME_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
	);
	const releaseResourceDirectory = path.join(
		externalResourcesStagingRoot,
		SSH_RUNTIME_RELEASE_DIRECTORY_NAME,
	);
	await requireMissing(trustDescriptorFilePath);
	await requireMissing(releaseResourceDirectory);

	await mkdir(path.dirname(trustDescriptorFilePath), { mode: 0o700, recursive: true });
	await mkdir(releaseResourceDirectory, { mode: 0o700, recursive: true });
	for (const artifact of verifiedRelease.artifacts) {
		const artifactPath = path.join(
			releaseResourceDirectory,
			SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS[artifact.platform],
		);
		await mkdir(path.dirname(artifactPath), { mode: 0o700, recursive: true });
		await writeFile(artifactPath, artifact.bytes, { mode: 0o500 });
	}
	await writeFile(
		path.join(releaseResourceDirectory, SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME),
		verifiedRelease.manifestBytes,
		{ mode: 0o400 },
	);
	await writeFile(
		trustDescriptorFilePath,
		encodeSshRuntimeReleaseTrustDescriptor(verifiedRelease.trustDescriptor),
		{ mode: 0o400 },
	);

	return {
		releaseResourceDirectory,
		trustDescriptor: verifiedRelease.trustDescriptor,
		trustDescriptorFilePath,
	};
}

export async function readVerifiedSshRuntimeRelease(options) {
	const releaseDirectory = requireAbsolutePath(
		options.releaseDirectory,
		'Runtime release directory',
	);
	const publicKeyFilePath = requireAbsolutePath(
		options.publicKeyFilePath,
		'Runtime public-key file',
	);
	await assertFinalizedLayout(releaseDirectory);
	const manifestBytes = await readRegularFile(
		path.join(releaseDirectory, SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME),
		SSH_RUNTIME_RELEASE_MAXIMUM_MANIFEST_BYTES,
		'Runtime release manifest',
	);
	const publicKeyBytes = await readRegularFile(
		publicKeyFilePath,
		MAXIMUM_PUBLIC_KEY_BYTES,
		'Runtime release public key',
	);
	const publicKeySpki = resolveCanonicalEd25519Spki(publicKeyBytes);
	const trustDescriptor = SshRuntimeReleaseTrustDescriptorSchema.parse({
		manifestSha256: digest(manifestBytes),
		publicKeySpki: publicKeySpki.toString('base64url'),
		schemaVersion: SSH_RUNTIME_RELEASE_TRUST_SCHEMA_VERSION,
		signingKeyId: `${SSH_RUNTIME_RELEASE_SIGNING_KEY_ID_PREFIX}${digest(publicKeySpki)}`,
	});
	const artifacts = [];
	for (const platform of SSH_RUNTIME_RELEASE_PLATFORMS) {
		const bytes = await readRegularFile(
			path.join(releaseDirectory, SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS[platform]),
			SSH_RUNTIME_RELEASE_MAXIMUM_ARTIFACT_BYTES,
			`Runtime release artifact for ${platform}`,
		);
		const verification = verifySshRuntimeArtifact({
			artifactBytes: bytes,
			expectedPlatform: platform,
			manifestBytes,
			trustDescriptor,
		});
		if (verification.kind === 'failed') {
			throw new Error(
				`Runtime release verification failed for ${platform}: ${verification.reason}`,
			);
		}
		artifacts.push({ bytes: Buffer.from(verification.artifact.bytes), platform });
	}
	return {
		artifacts,
		manifestBytes: Buffer.from(manifestBytes),
		trustDescriptor,
	};
}

async function assertFinalizedLayout(root) {
	await assertExactDirectory(root, ['artifacts', SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME]);
	const artifactsRoot = path.join(root, 'artifacts');
	await assertExactDirectory(artifactsRoot, [...SSH_RUNTIME_RELEASE_PLATFORMS]);
	for (const platform of SSH_RUNTIME_RELEASE_PLATFORMS) {
		await assertExactDirectory(path.join(artifactsRoot, platform), [
			SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME,
		]);
	}
}

async function assertExactDirectory(directory, expectedEntries) {
	const metadata = await lstat(directory);
	if (!metadata.isDirectory())
		throw new Error(`Runtime release path is not a directory: ${directory}`);
	const actual = (await readdir(directory)).toSorted();
	const expected = expectedEntries.toSorted();
	if (JSON.stringify(actual) !== JSON.stringify(expected)) {
		throw new Error(`Runtime release directory has an unexpected layout: ${directory}`);
	}
}

async function readRegularFile(filePath, maximumBytes, label) {
	const metadata = await lstat(filePath);
	if (!metadata.isFile()) throw new Error(`${label} is not a regular file: ${filePath}`);
	if (metadata.size > maximumBytes) throw new Error(`${label} exceeds its byte boundary.`);
	const bytes = await readFile(filePath);
	if (bytes.byteLength > maximumBytes) throw new Error(`${label} exceeds its byte boundary.`);
	return bytes;
}

function resolveCanonicalEd25519Spki(encoded) {
	let publicKey;
	try {
		publicKey = createPublicKey(encoded);
	} catch {
		try {
			publicKey = createPublicKey({ format: 'der', key: encoded, type: 'spki' });
		} catch {
			throw new Error('Runtime release public key is not a valid Ed25519 public key.');
		}
	}
	if (publicKey.asymmetricKeyType !== 'ed25519') {
		throw new Error('Runtime release public key is not Ed25519.');
	}
	const canonical = publicKey.export({ format: 'der', type: 'spki' });
	if (!Buffer.isBuffer(canonical)) {
		throw new Error('Runtime release public key did not produce canonical SPKI bytes.');
	}
	return canonical;
}

async function requireMissing(target) {
	try {
		await lstat(target);
	} catch (error) {
		if (isNodeError(error) && error.code === 'ENOENT') return;
		throw error;
	}
	throw new Error(`Runtime release package output already exists: ${target}`);
}

function requireAbsolutePath(value, label) {
	if (typeof value !== 'string' || !path.isAbsolute(value)) {
		throw new Error(`${label} must be an absolute path.`);
	}
	return path.normalize(value);
}

function digest(bytes) {
	return createHash('sha256').update(bytes).digest('hex');
}

function isNodeError(error) {
	return error instanceof Error && 'code' in error;
}
