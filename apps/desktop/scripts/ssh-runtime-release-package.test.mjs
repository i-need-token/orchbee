import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import { mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
	SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_RUNTIME_RELEASE_DIRECTORY_NAME,
	SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME,
	SSH_RUNTIME_RELEASE_PLATFORMS,
	SSH_RUNTIME_RELEASE_SCHEMA_VERSION,
	SSH_RUNTIME_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
	SshRuntimeReleaseManifestSchema,
	SshRuntimeReleaseTrustDescriptorSchema,
	createSshRuntimeManifestSigningPayload,
	encodeSshRuntimeReleaseManifest,
} from '@orchbee/ssh-environment-protocol';

import { verifySshRuntimeArtifact } from '../src/main/ssh-environment/verify-ssh-runtime-artifact.ts';
import {
	readVerifiedSshRuntimeRelease,
	resolveSshRuntimeReleasePackageConfiguration,
	stageSshRuntimeReleasePackageResources,
} from './ssh-runtime-release-package.mjs';

test('package mode keeps development release-free and requires two absolute release inputs', () => {
	const absoluteRoot = path.parse(process.cwd()).root;
	const publicKeyFilePath = path.join(absoluteRoot, 'release-inputs', 'public-key.pem');
	const releaseDirectory = path.join(absoluteRoot, 'release-inputs', 'final-release');
	const environment = {
		ORCHBEE_SSH_RUNTIME_RELEASE_DIRECTORY: releaseDirectory,
		ORCHBEE_SSH_RUNTIME_RELEASE_PUBLIC_KEY: publicKeyFilePath,
	};

	assert.equal(resolveSshRuntimeReleasePackageConfiguration([], environment), null);
	assert.deepEqual(
		resolveSshRuntimeReleasePackageConfiguration(['--with-ssh-runtime-release'], environment),
		{ publicKeyFilePath, releaseDirectory },
	);
	assert.throws(
		() => resolveSshRuntimeReleasePackageConfiguration(['--unsupported'], environment),
		/unsupported argument/u,
	);
	assert.throws(
		() =>
			resolveSshRuntimeReleasePackageConfiguration(['--with-ssh-runtime-release'], {
				...environment,
				ORCHBEE_SSH_RUNTIME_RELEASE_DIRECTORY: 'relative-release',
			}),
		/ORCHBEE_SSH_RUNTIME_RELEASE_DIRECTORY must be an absolute path/u,
	);
	assert.throws(
		() =>
			resolveSshRuntimeReleasePackageConfiguration(['--with-ssh-runtime-release'], {
				...environment,
				ORCHBEE_SSH_RUNTIME_RELEASE_PUBLIC_KEY: undefined,
			}),
		/ORCHBEE_SSH_RUNTIME_RELEASE_PUBLIC_KEY must be an absolute path/u,
	);
});

test('staging buffers one verified release and embeds its exact trust descriptor', async () => {
	await withTemporaryRoot(async (root) => {
		const fixture = await createReleaseFixture(root, 'valid');
		const applicationStagingRoot = path.join(root, 'application-staging');
		const externalResourcesStagingRoot = path.join(root, 'resources-staging');
		const result = await stageSshRuntimeReleasePackageResources({
			applicationStagingRoot,
			externalResourcesStagingRoot,
			publicKeyFilePath: fixture.publicKeyFilePath,
			releaseDirectory: fixture.releaseDirectory,
		});

		assert.equal(
			result.releaseResourceDirectory,
			path.join(externalResourcesStagingRoot, SSH_RUNTIME_RELEASE_DIRECTORY_NAME),
		);
		assert.deepEqual((await readdir(result.releaseResourceDirectory)).toSorted(), [
			'artifacts',
			SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME,
		]);
		const descriptorBytes = await readFile(
			path.join(
				applicationStagingRoot,
				'dist',
				'main',
				SSH_RUNTIME_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
			),
		);
		const descriptor = SshRuntimeReleaseTrustDescriptorSchema.parse(
			JSON.parse(descriptorBytes.toString('utf8')),
		);
		assert.deepEqual(descriptor, result.trustDescriptor);
		for (const platform of SSH_RUNTIME_RELEASE_PLATFORMS) {
			const stagedArtifact = await readFile(
				path.join(
					result.releaseResourceDirectory,
					SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS[platform],
				),
			);
			assert.equal(
				verifySshRuntimeArtifact({
					artifactBytes: stagedArtifact,
					expectedPlatform: platform,
					manifestBytes: fixture.manifestBytes,
					trustDescriptor: descriptor,
				}).kind,
				'verified',
			);
		}

		const sourceArtifactPath = path.join(
			fixture.releaseDirectory,
			SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['darwin-arm64'],
		);
		await writeFile(sourceArtifactPath, 'source changed after staging');
		assert.deepEqual(
			await readFile(
				path.join(
					result.releaseResourceDirectory,
					SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['darwin-arm64'],
				),
			),
			fixture.artifactBytes['darwin-arm64'],
		);
	});
});

test('verification rejects an independent wrong key and any signed artifact drift', async () => {
	await withTemporaryRoot(async (root) => {
		const wrongKeyFixture = await createReleaseFixture(root, 'wrong-key');
		const wrongSigner = generateKeyPairSync('ed25519');
		await writeFile(
			wrongKeyFixture.publicKeyFilePath,
			wrongSigner.publicKey.export({ format: 'pem', type: 'spki' }),
		);
		await assert.rejects(
			readVerifiedSshRuntimeRelease(wrongKeyFixture),
			/manifest-signature-invalid/u,
		);

		const driftFixture = await createReleaseFixture(root, 'artifact-drift');
		await writeFile(
			path.join(
				driftFixture.releaseDirectory,
				SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['linux-x64'],
			),
			'changed artifact',
		);
		await assert.rejects(
			readVerifiedSshRuntimeRelease(driftFixture),
			/artifact-(?:digest|size)-mismatch/u,
		);
	});
});

test('verification rejects noncanonical manifests, expanded layouts, and symlinks', async () => {
	await withTemporaryRoot(async (root) => {
		const noncanonicalFixture = await createReleaseFixture(root, 'noncanonical');
		await writeFile(
			path.join(noncanonicalFixture.releaseDirectory, SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME),
			JSON.stringify(JSON.parse(noncanonicalFixture.manifestBytes.toString('utf8'))),
		);
		await assert.rejects(readVerifiedSshRuntimeRelease(noncanonicalFixture), /manifest-invalid/u);

		const expandedFixture = await createReleaseFixture(root, 'expanded');
		await writeFile(path.join(expandedFixture.releaseDirectory, 'unexpected'), 'not admitted');
		await assert.rejects(readVerifiedSshRuntimeRelease(expandedFixture), /unexpected layout/u);

		const symlinkFixture = await createReleaseFixture(root, 'symlink');
		const artifactPath = path.join(
			symlinkFixture.releaseDirectory,
			SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['darwin-arm64'],
		);
		const symlinkTarget = path.join(root, 'symlink-target');
		await writeFile(symlinkTarget, 'not the signed artifact');
		await rm(artifactPath);
		await symlink(symlinkTarget, artifactPath);
		await assert.rejects(readVerifiedSshRuntimeRelease(symlinkFixture), /not a regular file/u);
	});
});

async function createReleaseFixture(root, name) {
	const releaseDirectory = path.join(root, `${name}-release`);
	const signer = generateKeyPairSync('ed25519');
	const artifactBytes = {
		'darwin-arm64': Buffer.from(`${name}:darwin-arm64:Runtime`),
		'linux-x64': Buffer.from(`${name}:linux-x64:Runtime`),
	};
	const unsignedManifest = {
		artifacts: SSH_RUNTIME_RELEASE_PLATFORMS.map((platform) => ({
			platform,
			sha256: digest(artifactBytes[platform]),
			sizeBytes: artifactBytes[platform].byteLength,
		})),
		runtimeVersion: '1.2.3',
		schemaVersion: SSH_RUNTIME_RELEASE_SCHEMA_VERSION,
	};
	const manifest = SshRuntimeReleaseManifestSchema.parse({
		...unsignedManifest,
		signature: sign(
			null,
			createSshRuntimeManifestSigningPayload(unsignedManifest),
			signer.privateKey,
		).toString('base64url'),
	});
	const manifestBytes = Buffer.from(encodeSshRuntimeReleaseManifest(manifest));
	for (const platform of SSH_RUNTIME_RELEASE_PLATFORMS) {
		const artifactPath = path.join(
			releaseDirectory,
			SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS[platform],
		);
		await mkdir(path.dirname(artifactPath), { recursive: true });
		await writeFile(artifactPath, artifactBytes[platform]);
	}
	await writeFile(
		path.join(releaseDirectory, SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME),
		manifestBytes,
	);
	const publicKeyFilePath = path.join(root, `${name}-public-key.pem`);
	await writeFile(publicKeyFilePath, signer.publicKey.export({ format: 'pem', type: 'spki' }));
	return { artifactBytes, manifestBytes, publicKeyFilePath, releaseDirectory };
}

async function withTemporaryRoot(operation) {
	const root = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-runtime-package-test-'));
	try {
		await operation(root);
	} finally {
		await rm(root, { force: true, recursive: true });
	}
}

function digest(bytes) {
	return createHash('sha256').update(bytes).digest('hex');
}
