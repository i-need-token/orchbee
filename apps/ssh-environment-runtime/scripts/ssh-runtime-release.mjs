import { createHash, createPublicKey, verify } from 'node:crypto';
import { lstat, mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { spawn } from 'node:child_process';

import {
	SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME,
	SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME,
	SSH_RUNTIME_RELEASE_MAXIMUM_ARTIFACT_BYTES,
	SSH_RUNTIME_RELEASE_MAXIMUM_MANIFEST_BYTES,
	SSH_RUNTIME_RELEASE_PLATFORMS,
	SSH_RUNTIME_RELEASE_SCHEMA_VERSION,
	SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME,
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentRuntimeVersionSchema,
	SshRuntimeReleaseManifestSchema,
	SshRuntimeReleaseUnsignedManifestSchema,
	createSshRuntimeManifestSigningPayload,
	encodeSshRuntimeReleaseManifest,
} from '@orchbee/ssh-environment-protocol';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });
const MAXIMUM_PUBLIC_KEY_BYTES = 16 * 1024;
const SSH_RUNTIME_RELEASE_BUILD_MATRIX = Object.freeze({
	'darwin-arm64': 'aarch64-macos',
	'linux-x64': 'x86_64-linux-gnu',
});

export async function prepareSshRuntimeRelease(options) {
	const runtimeRoot = requireAbsolutePath(options.runtimeRoot, 'Runtime root');
	const outputDirectory = requireOutputPath(options.outputDirectory);
	const runtimeVersion = SshEnvironmentRuntimeVersionSchema.parse(options.runtimeVersion);
	const buildTarget = options.buildTarget ?? buildRuntimeTarget;
	await requireMissing(outputDirectory);
	const workspace = await createPrivateWorkspace(outputDirectory, 'prepare');
	const candidate = path.join(workspace, 'prepared-release');
	try {
		const artifacts = [];
		for (const platform of SSH_RUNTIME_RELEASE_PLATFORMS) {
			const installationPrefix = path.join(workspace, 'build', platform);
			await buildTarget({
				installationPrefix,
				platform,
				runtimeRoot,
				runtimeVersion,
				target: SSH_RUNTIME_RELEASE_BUILD_MATRIX[platform],
			});
			const builtArtifactPath = path.join(
				installationPrefix,
				'bin',
				SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME,
			);
			const bytes = await readRegularFile(
				builtArtifactPath,
				SSH_RUNTIME_RELEASE_MAXIMUM_ARTIFACT_BYTES,
				'Runtime artifact',
			);
			if (bytes.byteLength === 0) throw new Error('Runtime artifact must not be empty.');
			artifacts.push({
				bytes,
				platform,
				sha256: digest(bytes),
				sizeBytes: bytes.byteLength,
			});
		}

		const unsignedManifest = SshRuntimeReleaseUnsignedManifestSchema.parse({
			artifacts: artifacts.map(({ platform, sha256, sizeBytes }) => ({
				platform,
				sha256,
				sizeBytes,
			})),
			runtimeVersion,
			schemaVersion: SSH_RUNTIME_RELEASE_SCHEMA_VERSION,
		});
		await writeReleaseArtifacts(candidate, artifacts);
		await writeFile(
			path.join(candidate, SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME),
			createSshRuntimeManifestSigningPayload(unsignedManifest),
			{ mode: 0o600 },
		);
		await requireMissing(outputDirectory);
		await rename(candidate, outputDirectory);
		return { outputDirectory, unsignedManifest };
	} finally {
		await rm(workspace, { force: true, recursive: true });
	}
}

export async function finalizeSshRuntimeRelease(options) {
	const preparedDirectory = requireAbsolutePath(options.preparedDirectory, 'Prepared directory');
	const signatureFilePath = requireAbsolutePath(options.signatureFilePath, 'Signature file');
	const publicKeyFilePath = requireAbsolutePath(options.publicKeyFilePath, 'Public-key file');
	const outputDirectory = requireOutputPath(options.outputDirectory);
	await requireMissing(outputDirectory);
	await assertPreparedLayout(preparedDirectory);

	const payloadBytes = await readRegularFile(
		path.join(preparedDirectory, SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME),
		SSH_RUNTIME_RELEASE_MAXIMUM_MANIFEST_BYTES,
		'Manifest signing payload',
	);
	const unsignedManifest = decodeCanonicalUnsignedManifest(payloadBytes);
	const artifacts = await readAndVerifyArtifacts(preparedDirectory, unsignedManifest.artifacts);
	const signatureBytes = await readRegularFile(signatureFilePath, 64, 'Ed25519 signature');
	if (signatureBytes.byteLength !== 64) {
		throw new Error('Ed25519 signature must contain exactly 64 raw bytes.');
	}
	const publicKeyBytes = await readRegularFile(
		publicKeyFilePath,
		MAXIMUM_PUBLIC_KEY_BYTES,
		'Ed25519 public key',
	);
	const publicKey = resolveEd25519PublicKey(publicKeyBytes);
	if (!verify(null, payloadBytes, publicKey, signatureBytes)) {
		throw new Error('Ed25519 signature does not authenticate the prepared Runtime release.');
	}

	const manifest = SshRuntimeReleaseManifestSchema.parse({
		...unsignedManifest,
		signature: signatureBytes.toString('base64url'),
	});
	const workspace = await createPrivateWorkspace(outputDirectory, 'finalize');
	const candidate = path.join(workspace, 'final-release');
	try {
		await writeReleaseArtifacts(candidate, artifacts);
		await writeFile(
			path.join(candidate, SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME),
			encodeSshRuntimeReleaseManifest(manifest),
			{ mode: 0o600 },
		);
		await requireMissing(outputDirectory);
		await rename(candidate, outputDirectory);
		return { manifest, outputDirectory };
	} finally {
		await rm(workspace, { force: true, recursive: true });
	}
}

export async function buildRuntimeTarget(options) {
	await runProcess('zig', createSshRuntimeReleaseBuildArguments(options), options.runtimeRoot);
}

export function createSshRuntimeReleaseBuildArguments(options) {
	return [
		'build',
		`-Dtarget=${options.target}`,
		'-Doptimize=ReleaseSafe',
		'-Dstrip=true',
		`-Druntime-version=${options.runtimeVersion}`,
		'--prefix',
		options.installationPrefix,
	];
}

async function assertPreparedLayout(root) {
	await assertExactDirectory(root, ['artifacts', SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME]);
	await assertArtifactDirectories(root);
}

async function assertArtifactDirectories(root) {
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
	if (!metadata.isDirectory()) throw new Error(`Release path is not a directory: ${directory}`);
	const actual = (await readdir(directory)).toSorted();
	const expected = expectedEntries.toSorted();
	if (JSON.stringify(actual) !== JSON.stringify(expected)) {
		throw new Error(`Release directory has an unexpected layout: ${directory}`);
	}
}

async function readAndVerifyArtifacts(root, manifestArtifacts) {
	const artifacts = [];
	for (const releaseArtifact of manifestArtifacts) {
		const artifactPath = path.join(
			root,
			SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS[releaseArtifact.platform],
		);
		const bytes = await readRegularFile(
			artifactPath,
			SSH_RUNTIME_RELEASE_MAXIMUM_ARTIFACT_BYTES,
			'Runtime artifact',
		);
		if (
			bytes.byteLength !== releaseArtifact.sizeBytes ||
			digest(bytes) !== releaseArtifact.sha256
		) {
			throw new Error(
				`Runtime artifact does not match its signed payload: ${releaseArtifact.platform}`,
			);
		}
		artifacts.push({ ...releaseArtifact, bytes });
	}
	return artifacts;
}

function decodeCanonicalUnsignedManifest(payloadBytes) {
	if (payloadBytes.byteLength === 0) throw new Error('Manifest signing payload must not be empty.');
	let value;
	try {
		value = JSON.parse(utf8Decoder.decode(payloadBytes));
	} catch {
		throw new Error('Manifest signing payload is not strict UTF-8 JSON.');
	}
	const parsed = SshRuntimeReleaseUnsignedManifestSchema.safeParse(value);
	if (!parsed.success) throw new Error('Manifest signing payload violates the release contract.');
	if (
		!Buffer.from(createSshRuntimeManifestSigningPayload(parsed.data)).equals(
			Buffer.from(payloadBytes),
		)
	) {
		throw new Error('Manifest signing payload is not canonically encoded.');
	}
	return parsed.data;
}

function resolveEd25519PublicKey(encoded) {
	try {
		const publicKey = createPublicKey(encoded);
		if (publicKey.asymmetricKeyType !== 'ed25519') {
			throw new Error('Release public key is not Ed25519.');
		}
		return publicKey;
	} catch (error) {
		if (error instanceof Error && error.message === 'Release public key is not Ed25519.')
			throw error;
		throw new Error('Release public key is not a valid Ed25519 public key.');
	}
}

async function writeReleaseArtifacts(root, artifacts) {
	await mkdir(root, { mode: 0o700, recursive: true });
	for (const artifact of artifacts) {
		const artifactPath = path.join(
			root,
			SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS[artifact.platform],
		);
		await mkdir(path.dirname(artifactPath), { mode: 0o700, recursive: true });
		await writeFile(artifactPath, artifact.bytes, { mode: 0o500 });
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

async function createPrivateWorkspace(outputDirectory, operation) {
	const parent = path.dirname(outputDirectory);
	await mkdir(parent, { mode: 0o700, recursive: true });
	return mkdtemp(path.join(parent, `.${path.basename(outputDirectory)}.${operation}-`));
}

async function requireMissing(target) {
	try {
		await lstat(target);
	} catch (error) {
		if (isNodeError(error) && error.code === 'ENOENT') return;
		throw error;
	}
	throw new Error(`Release output already exists: ${target}`);
}

function requireOutputPath(value) {
	const output = requireAbsolutePath(value, 'Release output');
	if (path.parse(output).root === output)
		throw new Error('Release output cannot be a filesystem root.');
	return output;
}

function requireAbsolutePath(value, label) {
	if (typeof value !== 'string' || !path.isAbsolute(value)) {
		throw new Error(`${label} must be an absolute path.`);
	}
	return path.normalize(value);
}

function digest(bytes) {
	return SshEnvironmentBuildDigestSchema.parse(createHash('sha256').update(bytes).digest('hex'));
}

function runProcess(command, arguments_, workingDirectory) {
	return new Promise((resolve, reject) => {
		const child = spawn(command, arguments_, {
			cwd: workingDirectory,
			shell: false,
			stdio: 'inherit',
		});
		child.once('error', reject);
		child.once('exit', (code, signal) => {
			if (code === 0 && signal === null) resolve();
			else
				reject(
					new Error(`${command} failed with code ${String(code)} and signal ${String(signal)}.`),
				);
		});
	});
}

function isNodeError(error) {
	return error instanceof Error && 'code' in error;
}
