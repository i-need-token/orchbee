import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, sign, verify } from 'node:crypto';
import {
	chmod,
	lstat,
	mkdir,
	mkdtemp,
	readFile,
	readdir,
	rm,
	symlink,
	writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
	SSH_ENVIRONMENT_RELEASE_ARTIFACT_FILE_NAMES,
	SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES,
	SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME,
	SSH_ENVIRONMENT_RELEASE_PLATFORMS,
	SSH_ENVIRONMENT_RELEASE_SIGNING_PAYLOAD_FILE_NAME,
	SshEnvironmentReleaseManifestSchema,
	createSshEnvironmentReleaseManifestSigningPayload,
} from '@orchbee/ssh-environment-protocol';

import {
	createSshEnvironmentRuntimeReleaseBuildArguments,
	createTerminalHostReleaseBuildArguments,
	finalizeSshEnvironmentRelease,
	prepareSshEnvironmentRelease,
} from './ssh-environment-release.mjs';

test('production component builds pin target, version, optimization, and stripping', () => {
	assert.deepEqual(
		createSshEnvironmentRuntimeReleaseBuildArguments({
			installationPrefix: '/private/runtime-prefix',
			target: 'aarch64-macos',
			version: '1.2.3',
		}),
		[
			'build',
			'-Dtarget=aarch64-macos',
			'-Doptimize=ReleaseSafe',
			'-Dstrip=true',
			'-Druntime-version=1.2.3',
			'--prefix',
			'/private/runtime-prefix',
		],
	);
	assert.deepEqual(
		createTerminalHostReleaseBuildArguments({
			installationPrefix: '/private/terminal-prefix',
			target: 'x86_64-linux-gnu',
			version: '4.5.6',
		}),
		[
			'build',
			'-Dtarget=x86_64-linux-gnu',
			'-Doptimize=ReleaseSafe',
			'-Dstrip=true',
			'-Dhost-version=4.5.6',
			'--prefix',
			'/private/terminal-prefix',
		],
	);
});

test('prepare builds the exact component matrix and emits only canonical signing inputs', async () => {
	await withTemporaryRoot(async (root) => {
		const calls = [];
		const outputDirectory = path.join(root, 'prepared');
		const result = await prepareSshEnvironmentRelease({
			buildComponent: async (options) => {
				calls.push(options);
				await writeFakeBuild(options.installationPrefix, options.role, options.platform);
			},
			outputDirectory,
			runtimeRoot: path.join(root, 'runtime-source'),
			runtimeVersion: '1.2.3',
			terminalHostRoot: path.join(root, 'terminal-source'),
			terminalHostVersion: '4.5.6',
		});

		assert.deepEqual(
			calls.map(({ platform, role, target, version }) => ({ platform, role, target, version })),
			[
				{
					platform: 'darwin-arm64',
					role: 'ssh-environment-runtime',
					target: 'aarch64-macos',
					version: '1.2.3',
				},
				{
					platform: 'linux-x64',
					role: 'ssh-environment-runtime',
					target: 'x86_64-linux-gnu',
					version: '1.2.3',
				},
				{
					platform: 'darwin-arm64',
					role: 'terminal-host',
					target: 'aarch64-macos',
					version: '4.5.6',
				},
				{
					platform: 'linux-x64',
					role: 'terminal-host',
					target: 'x86_64-linux-gnu',
					version: '4.5.6',
				},
			],
		);
		assert.deepEqual((await readdir(outputDirectory)).toSorted(), [
			'artifacts',
			SSH_ENVIRONMENT_RELEASE_SIGNING_PAYLOAD_FILE_NAME,
		]);
		for (const platform of SSH_ENVIRONMENT_RELEASE_PLATFORMS) {
			assert.deepEqual(
				(await readdir(path.join(outputDirectory, 'artifacts', platform))).toSorted(),
				SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES.map(
					(role) => SSH_ENVIRONMENT_RELEASE_ARTIFACT_FILE_NAMES[role],
				).toSorted(),
			);
		}
		const payload = await readFile(
			path.join(outputDirectory, SSH_ENVIRONMENT_RELEASE_SIGNING_PAYLOAD_FILE_NAME),
		);
		assert.deepEqual(
			payload,
			Buffer.from(createSshEnvironmentReleaseManifestSigningPayload(result.unsignedManifest)),
		);
	});
});

test('finalize verifies one external signature and publishes the exact four-artifact bundle', async () => {
	await withTemporaryRoot(async (root) => {
		const preparedDirectory = await prepareFakeRelease(root);
		const payload = await readFile(
			path.join(preparedDirectory, SSH_ENVIRONMENT_RELEASE_SIGNING_PAYLOAD_FILE_NAME),
		);
		const signer = generateKeyPairSync('ed25519');
		const signatureFilePath = path.join(root, 'signature.bin');
		const publicKeyFilePath = path.join(root, 'public-key.pem');
		await writeFile(signatureFilePath, sign(null, payload, signer.privateKey));
		await writeFile(publicKeyFilePath, signer.publicKey.export({ format: 'pem', type: 'spki' }));
		const outputDirectory = path.join(root, 'release');

		const result = await finalizeSshEnvironmentRelease({
			outputDirectory,
			preparedDirectory,
			publicKeyFilePath,
			signatureFilePath,
		});
		const manifestBytes = await readFile(
			path.join(outputDirectory, SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME),
		);
		const manifest = SshEnvironmentReleaseManifestSchema.parse(
			JSON.parse(manifestBytes.toString('utf8')),
		);
		const { signature, ...unsignedManifest } = manifest;
		assert.deepEqual(result.manifest, manifest);
		assert.equal(
			verify(
				null,
				createSshEnvironmentReleaseManifestSigningPayload(unsignedManifest),
				signer.publicKey,
				Buffer.from(signature, 'base64url'),
			),
			true,
		);
		assert.deepEqual((await readdir(outputDirectory)).toSorted(), [
			'artifacts',
			SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME,
		]);
	});
});

test('finalize rejects wrong signatures, component drift, symlinks, and expanded layouts', async () => {
	await withTemporaryRoot(async (root) => {
		const preparedDirectory = await prepareFakeRelease(root);
		const payload = await readFile(
			path.join(preparedDirectory, SSH_ENVIRONMENT_RELEASE_SIGNING_PAYLOAD_FILE_NAME),
		);
		const signer = generateKeyPairSync('ed25519');
		const otherSigner = generateKeyPairSync('ed25519');
		const publicKeyFilePath = path.join(root, 'public-key.pem');
		const signatureFilePath = path.join(root, 'signature.bin');
		await writeFile(publicKeyFilePath, signer.publicKey.export({ format: 'pem', type: 'spki' }));
		await writeFile(signatureFilePath, sign(null, payload, otherSigner.privateKey));
		await assert.rejects(
			finalizeSshEnvironmentRelease({
				outputDirectory: path.join(root, 'wrong-signature-release'),
				preparedDirectory,
				publicKeyFilePath,
				signatureFilePath,
			}),
			/does not authenticate/u,
		);

		await writeFile(signatureFilePath, sign(null, payload, signer.privateKey));
		const terminalHostPath = path.join(
			preparedDirectory,
			SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS['terminal-host']['darwin-arm64'],
		);
		await chmod(terminalHostPath, 0o600);
		await writeFile(terminalHostPath, 'mutated');
		await assert.rejects(
			finalizeSshEnvironmentRelease({
				outputDirectory: path.join(root, 'mutated-release'),
				preparedDirectory,
				publicKeyFilePath,
				signatureFilePath,
			}),
			/does not match/u,
		);

		await rm(terminalHostPath);
		const symlinkTarget = path.join(root, 'terminal-host-symlink-target');
		await writeFile(symlinkTarget, 'not the signed artifact');
		await symlink(symlinkTarget, terminalHostPath);
		await assert.rejects(
			finalizeSshEnvironmentRelease({
				outputDirectory: path.join(root, 'symlink-release'),
				preparedDirectory,
				publicKeyFilePath,
				signatureFilePath,
			}),
			/not a regular file/u,
		);

		await rm(terminalHostPath);
		await writeFile(terminalHostPath, 'restored-but-invalid');
		await writeFile(path.join(preparedDirectory, 'unexpected'), 'not admitted');
		await assert.rejects(
			finalizeSshEnvironmentRelease({
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
	await prepareSshEnvironmentRelease({
		buildComponent: async (options) =>
			writeFakeBuild(options.installationPrefix, options.role, options.platform),
		outputDirectory,
		runtimeRoot: path.join(root, 'runtime-source'),
		runtimeVersion: '1.2.3',
		terminalHostRoot: path.join(root, 'terminal-source'),
		terminalHostVersion: '4.5.6',
	});
	return outputDirectory;
}

async function writeFakeBuild(installationPrefix, role, platform) {
	const binaryDirectory = path.join(installationPrefix, 'bin');
	await mkdir(binaryDirectory, { recursive: true });
	await writeFile(
		path.join(binaryDirectory, SSH_ENVIRONMENT_RELEASE_ARTIFACT_FILE_NAMES[role]),
		Buffer.from(
			`${role}:${platform}:${createHash('sha256').update(`${role}:${platform}`).digest('hex')}`,
		),
	);
}

async function withTemporaryRoot(operation) {
	const root = await mkdtemp(path.join(tmpdir(), 'orchbee-ssh-environment-release-test-'));
	try {
		await operation(root);
	} finally {
		await rm(root, { force: true, recursive: true });
	}
}
