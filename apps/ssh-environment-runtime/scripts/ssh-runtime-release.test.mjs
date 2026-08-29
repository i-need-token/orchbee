import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, sign, verify } from 'node:crypto';
import { chmod, lstat, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
	SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME,
	SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME,
	SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME,
	SshRuntimeReleaseManifestSchema,
	createSshRuntimeManifestSigningPayload,
} from '@orchbee/ssh-environment-protocol';

import {
	createSshRuntimeReleaseBuildArguments,
	finalizeSshRuntimeRelease,
	prepareSshRuntimeRelease,
} from './ssh-runtime-release.mjs';

test('the production build command pins the target, version, optimization, and stripping', () => {
	assert.deepEqual(
		createSshRuntimeReleaseBuildArguments({
			installationPrefix: '/private/release-prefix',
			runtimeVersion: '1.2.3',
			target: 'aarch64-macos',
		}),
		[
			'build',
			'-Dtarget=aarch64-macos',
			'-Doptimize=ReleaseSafe',
			'-Dstrip=true',
			'-Druntime-version=1.2.3',
			'--prefix',
			'/private/release-prefix',
		],
	);
});

test('prepare builds the exact stripped matrix and emits only canonical signing inputs', async () => {
	await withTemporaryRoot(async (root) => {
		const calls = [];
		const outputDirectory = path.join(root, 'prepared');
		const result = await prepareSshRuntimeRelease({
			buildTarget: async (options) => {
				calls.push(options);
				await writeFakeBuild(options.installationPrefix, options.platform);
			},
			outputDirectory,
			runtimeRoot: path.join(root, 'runtime-source'),
			runtimeVersion: '1.2.3',
		});

		assert.equal(result.outputDirectory, outputDirectory);
		assert.deepEqual(
			calls.map(({ platform, target }) => ({ platform, target })),
			[
				{ platform: 'darwin-arm64', target: 'aarch64-macos' },
				{ platform: 'linux-x64', target: 'x86_64-linux-gnu' },
			],
		);
		const rootEntries = (await readdir(outputDirectory)).toSorted();
		assert.deepEqual(rootEntries, ['artifacts', SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME]);
		const payload = await readFile(
			path.join(outputDirectory, SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME),
		);
		assert.deepEqual(
			payload,
			Buffer.from(createSshRuntimeManifestSigningPayload(result.unsignedManifest)),
		);
	});
});

test('finalize verifies an external signature and publishes one exact signed bundle', async () => {
	await withTemporaryRoot(async (root) => {
		const preparedDirectory = await prepareFakeRelease(root);
		const payload = await readFile(
			path.join(preparedDirectory, SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME),
		);
		const signer = generateKeyPairSync('ed25519');
		const signatureFilePath = path.join(root, 'signature.bin');
		const publicKeyFilePath = path.join(root, 'public-key.pem');
		await writeFile(signatureFilePath, sign(null, payload, signer.privateKey));
		await writeFile(publicKeyFilePath, signer.publicKey.export({ format: 'pem', type: 'spki' }));
		const outputDirectory = path.join(root, 'release');

		const result = await finalizeSshRuntimeRelease({
			outputDirectory,
			preparedDirectory,
			publicKeyFilePath,
			signatureFilePath,
		});
		const manifestBytes = await readFile(
			path.join(outputDirectory, SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME),
		);
		const manifest = SshRuntimeReleaseManifestSchema.parse(
			JSON.parse(manifestBytes.toString('utf8')),
		);
		const { signature, ...unsignedManifest } = manifest;
		assert.deepEqual(result.manifest, manifest);
		assert.equal(
			verify(
				null,
				createSshRuntimeManifestSigningPayload(unsignedManifest),
				signer.publicKey,
				Buffer.from(signature, 'base64url'),
			),
			true,
		);
		assert.deepEqual((await readdir(outputDirectory)).toSorted(), [
			'artifacts',
			SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME,
		]);
	});
});

test('finalize rejects artifact drift, wrong signatures, and unexpected prepared entries', async () => {
	await withTemporaryRoot(async (root) => {
		const preparedDirectory = await prepareFakeRelease(root);
		const payload = await readFile(
			path.join(preparedDirectory, SSH_RUNTIME_RELEASE_SIGNING_PAYLOAD_FILE_NAME),
		);
		const signer = generateKeyPairSync('ed25519');
		const otherSigner = generateKeyPairSync('ed25519');
		const publicKeyFilePath = path.join(root, 'public-key.pem');
		await writeFile(publicKeyFilePath, signer.publicKey.export({ format: 'pem', type: 'spki' }));
		const signatureFilePath = path.join(root, 'signature.bin');
		await writeFile(signatureFilePath, sign(null, payload, otherSigner.privateKey));

		await assert.rejects(
			finalizeSshRuntimeRelease({
				outputDirectory: path.join(root, 'wrong-signature-release'),
				preparedDirectory,
				publicKeyFilePath,
				signatureFilePath,
			}),
			/does not authenticate/u,
		);
		await writeFile(signatureFilePath, sign(null, payload, signer.privateKey));
		const artifactPath = path.join(
			preparedDirectory,
			SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['darwin-arm64'],
		);
		await chmod(artifactPath, 0o600);
		await writeFile(artifactPath, 'mutated');
		await assert.rejects(
			finalizeSshRuntimeRelease({
				outputDirectory: path.join(root, 'mutated-release'),
				preparedDirectory,
				publicKeyFilePath,
				signatureFilePath,
			}),
			/does not match/u,
		);
		await writeFile(path.join(preparedDirectory, 'unexpected'), 'not admitted');
		await assert.rejects(
			finalizeSshRuntimeRelease({
				outputDirectory: path.join(root, 'expanded-release'),
				preparedDirectory,
				publicKeyFilePath,
				signatureFilePath,
			}),
			/unexpected layout/u,
		);
		await assert.rejects(lstat(path.join(root, 'expanded-release')), { code: 'ENOENT' });
	});
});

async function prepareFakeRelease(root) {
	const outputDirectory = path.join(root, 'prepared');
	await prepareSshRuntimeRelease({
		buildTarget: async (options) => writeFakeBuild(options.installationPrefix, options.platform),
		outputDirectory,
		runtimeRoot: path.join(root, 'runtime-source'),
		runtimeVersion: '1.2.3',
	});
	return outputDirectory;
}

async function writeFakeBuild(installationPrefix, platform) {
	const binaryDirectory = path.join(installationPrefix, 'bin');
	await mkdir(binaryDirectory, { recursive: true });
	await writeFile(
		path.join(binaryDirectory, SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME),
		Buffer.from(`${platform} Runtime ${createHash('sha256').update(platform).digest('hex')}`),
	);
}

async function withTemporaryRoot(operation) {
	const root = await mkdtemp(path.join(tmpdir(), 'orchbee-runtime-release-test-'));
	try {
		await operation(root);
	} finally {
		await rm(root, { force: true, recursive: true });
	}
}
