import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import { mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
	SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES,
	SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME,
	SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME,
	SSH_ENVIRONMENT_RELEASE_PLATFORMS,
	SSH_ENVIRONMENT_RELEASE_SCHEMA_VERSION,
	SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
	SshEnvironmentReleaseManifestSchema,
	SshEnvironmentReleaseTrustDescriptorSchema,
	createSshEnvironmentReleaseManifestSigningPayload,
	encodeSshEnvironmentReleaseManifest,
} from '@orchbee/ssh-environment-protocol';

import {
	verifySshEnvironmentArtifactAgainstCatalog,
	verifySshEnvironmentReleaseCatalog,
} from '../src/main/ssh-environment/verify-ssh-environment-release.ts';
import {
	readVerifiedSshEnvironmentRelease,
	resolveSshEnvironmentReleasePackageConfiguration,
	stageSshEnvironmentReleasePackageResources,
} from './ssh-environment-release-package.mjs';

test('release-free packaging selects no external payload while Release requires two absolute inputs', () => {
	const absoluteRoot = path.parse(process.cwd()).root;
	const publicKeyFilePath = path.join(absoluteRoot, 'release-inputs', 'public-key.pem');
	const releaseDirectory = path.join(absoluteRoot, 'release-inputs', 'final-release');
	const environment = {
		ORCHBEE_SSH_ENVIRONMENT_RELEASE_DIRECTORY: releaseDirectory,
		ORCHBEE_SSH_ENVIRONMENT_RELEASE_PUBLIC_KEY: publicKeyFilePath,
	};

	assert.equal(resolveSshEnvironmentReleasePackageConfiguration([], environment), null);
	assert.deepEqual(
		resolveSshEnvironmentReleasePackageConfiguration(
			['--with-ssh-environment-release'],
			environment,
		),
		{ publicKeyFilePath, releaseDirectory },
	);
	assert.throws(
		() => resolveSshEnvironmentReleasePackageConfiguration(['--unsupported'], environment),
		/unsupported argument/u,
	);
	assert.throws(
		() =>
			resolveSshEnvironmentReleasePackageConfiguration(['--with-ssh-environment-release'], {
				...environment,
				ORCHBEE_SSH_ENVIRONMENT_RELEASE_DIRECTORY: 'relative-release',
			}),
		/ORCHBEE_SSH_ENVIRONMENT_RELEASE_DIRECTORY must be an absolute path/u,
	);
	assert.throws(
		() =>
			resolveSshEnvironmentReleasePackageConfiguration(['--with-ssh-environment-release'], {
				...environment,
				ORCHBEE_SSH_ENVIRONMENT_RELEASE_PUBLIC_KEY: undefined,
			}),
		/ORCHBEE_SSH_ENVIRONMENT_RELEASE_PUBLIC_KEY must be an absolute path/u,
	);
});

test('staging buffers the complete verified cohort and embeds its exact trust descriptor', async () => {
	await withTemporaryRoot(async (root) => {
		const fixture = await createReleaseFixture(root, 'valid');
		const applicationStagingRoot = path.join(root, 'application-staging');
		const externalResourcesStagingRoot = path.join(root, 'resources-staging');
		const result = await stageSshEnvironmentReleasePackageResources({
			applicationStagingRoot,
			externalResourcesStagingRoot,
			publicKeyFilePath: fixture.publicKeyFilePath,
			releaseDirectory: fixture.releaseDirectory,
		});

		assert.equal(
			result.releaseResourceDirectory,
			path.join(externalResourcesStagingRoot, SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME),
		);
		assert.deepEqual((await readdir(result.releaseResourceDirectory)).toSorted(), [
			'artifacts',
			SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME,
		]);
		const descriptorBytes = await readFile(
			path.join(
				applicationStagingRoot,
				'dist',
				'main',
				SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
			),
		);
		const descriptor = SshEnvironmentReleaseTrustDescriptorSchema.parse(
			JSON.parse(descriptorBytes.toString('utf8')),
		);
		assert.deepEqual(descriptor, result.trustDescriptor);
		for (const role of SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES) {
			for (const platform of SSH_ENVIRONMENT_RELEASE_PLATFORMS) {
				const stagedArtifact = await readFile(
					path.join(
						result.releaseResourceDirectory,
						SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS[role][platform],
					),
				);
				assert.equal(
					verifySshEnvironmentArtifact({
						artifactBytes: stagedArtifact,
						expectedPlatform: platform,
						expectedRole: role,
						manifestBytes: fixture.manifestBytes,
						trustDescriptor: descriptor,
					}).kind,
					'verified',
				);
			}
		}

		const sourceArtifactPath = path.join(
			fixture.releaseDirectory,
			SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS['terminal-host']['darwin-arm64'],
		);
		await writeFile(sourceArtifactPath, 'source changed after staging');
		assert.deepEqual(
			await readFile(
				path.join(
					result.releaseResourceDirectory,
					SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS['terminal-host']['darwin-arm64'],
				),
			),
			fixture.artifactBytes['terminal-host']['darwin-arm64'],
		);
	});
});

test('verification rejects a wrong key and drift in either signed component', async () => {
	await withTemporaryRoot(async (root) => {
		const wrongKeyFixture = await createReleaseFixture(root, 'wrong-key');
		const wrongSigner = generateKeyPairSync('ed25519');
		await writeFile(
			wrongKeyFixture.publicKeyFilePath,
			wrongSigner.publicKey.export({ format: 'pem', type: 'spki' }),
		);
		await assert.rejects(
			readVerifiedSshEnvironmentRelease(wrongKeyFixture),
			/manifest-signature-invalid/u,
		);

		for (const role of SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES) {
			const driftFixture = await createReleaseFixture(root, `artifact-drift-${role}`);
			await writeFile(
				path.join(
					driftFixture.releaseDirectory,
					SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS[role]['linux-x64'],
				),
				'changed artifact',
			);
			await assert.rejects(
				readVerifiedSshEnvironmentRelease(driftFixture),
				/artifact-(?:digest|size)-mismatch/u,
			);
		}
	});
});

test('verification rejects noncanonical manifests, expanded layouts, and symlinks', async () => {
	await withTemporaryRoot(async (root) => {
		const noncanonicalFixture = await createReleaseFixture(root, 'noncanonical');
		await writeFile(
			path.join(noncanonicalFixture.releaseDirectory, SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME),
			JSON.stringify(JSON.parse(noncanonicalFixture.manifestBytes.toString('utf8'))),
		);
		await assert.rejects(
			readVerifiedSshEnvironmentRelease(noncanonicalFixture),
			/manifest-invalid/u,
		);

		const expandedFixture = await createReleaseFixture(root, 'expanded');
		await writeFile(path.join(expandedFixture.releaseDirectory, 'unexpected'), 'not admitted');
		await assert.rejects(readVerifiedSshEnvironmentRelease(expandedFixture), /unexpected layout/u);

		const symlinkFixture = await createReleaseFixture(root, 'symlink');
		const artifactPath = path.join(
			symlinkFixture.releaseDirectory,
			SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS['terminal-host']['darwin-arm64'],
		);
		const symlinkTarget = path.join(root, 'symlink-target');
		await writeFile(symlinkTarget, 'not the signed artifact');
		await rm(artifactPath);
		await symlink(symlinkTarget, artifactPath);
		await assert.rejects(readVerifiedSshEnvironmentRelease(symlinkFixture), /not a regular file/u);
	});
});

function verifySshEnvironmentArtifact(options) {
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

async function createReleaseFixture(root, name) {
	const releaseDirectory = path.join(root, `${name}-release`);
	const signer = generateKeyPairSync('ed25519');
	const artifactBytes = {
		'ssh-environment-runtime': {
			'darwin-arm64': Buffer.from(`${name}:darwin-arm64:Runtime`),
			'linux-x64': Buffer.from(`${name}:linux-x64:Runtime`),
		},
		'terminal-host': {
			'darwin-arm64': Buffer.from(`${name}:darwin-arm64:TerminalHost`),
			'linux-x64': Buffer.from(`${name}:linux-x64:TerminalHost`),
		},
	};
	const unsignedManifest = {
		components: [
			{
				role: 'ssh-environment-runtime',
				version: '1.2.3',
				artifacts: createArtifactRecords(artifactBytes['ssh-environment-runtime']),
			},
			{
				role: 'terminal-host',
				version: '4.5.6',
				artifacts: createArtifactRecords(artifactBytes['terminal-host']),
			},
		],
		schemaVersion: SSH_ENVIRONMENT_RELEASE_SCHEMA_VERSION,
	};
	const manifest = SshEnvironmentReleaseManifestSchema.parse({
		...unsignedManifest,
		signature: sign(
			null,
			createSshEnvironmentReleaseManifestSigningPayload(unsignedManifest),
			signer.privateKey,
		).toString('base64url'),
	});
	const manifestBytes = Buffer.from(encodeSshEnvironmentReleaseManifest(manifest));
	for (const role of SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES) {
		for (const platform of SSH_ENVIRONMENT_RELEASE_PLATFORMS) {
			const artifactPath = path.join(
				releaseDirectory,
				SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS[role][platform],
			);
			await mkdir(path.dirname(artifactPath), { recursive: true });
			await writeFile(artifactPath, artifactBytes[role][platform]);
		}
	}
	await writeFile(
		path.join(releaseDirectory, SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME),
		manifestBytes,
	);
	const publicKeyFilePath = path.join(root, `${name}-public-key.pem`);
	await writeFile(publicKeyFilePath, signer.publicKey.export({ format: 'pem', type: 'spki' }));
	return { artifactBytes, manifestBytes, publicKeyFilePath, releaseDirectory };
}

function createArtifactRecords(componentBytes) {
	return SSH_ENVIRONMENT_RELEASE_PLATFORMS.map((platform) => ({
		platform,
		sha256: digest(componentBytes[platform]),
		sizeBytes: componentBytes[platform].byteLength,
	}));
}

async function withTemporaryRoot(operation) {
	const root = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-environment-package-test-'));
	try {
		await operation(root);
	} finally {
		await rm(root, { force: true, recursive: true });
	}
}

function digest(bytes) {
	return createHash('sha256').update(bytes).digest('hex');
}
