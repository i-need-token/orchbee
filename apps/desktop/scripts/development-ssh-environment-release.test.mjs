import assert from 'node:assert/strict';
import { chmod, mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
	SSH_ENVIRONMENT_RELEASE_ARTIFACT_FILE_NAMES,
	SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME,
	SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
} from '@orchbee/ssh-environment-protocol';

import { prepareDevelopmentSshEnvironmentRelease } from './development-ssh-environment-release.mjs';

test('development release cache reuses verified content identities and versions changed inputs', async () => {
	const fixture = await createFixture();
	try {
		const builds = [];
		const dependencies = createDependencies(builds);
		const first = await prepareDevelopmentSshEnvironmentRelease(
			{ outputRoot: fixture.outputRoot, sourceRoot: fixture.sourceRoot },
			dependencies,
		);
		const reused = await prepareDevelopmentSshEnvironmentRelease(
			{ outputRoot: fixture.outputRoot, sourceRoot: fixture.sourceRoot },
			dependencies,
		);
		assert.deepEqual(reused, first);
		assert.equal(builds.length, 4);
		assert.deepEqual(
			(await readdir(path.dirname(first.releaseDirectoryPath))).toSorted(),
			[
				'development-release-cache-v1.json',
				'development-signing-public-key.pem',
				SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME,
				SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
			].toSorted(),
		);

		await writeFile(
			path.join(fixture.sourceRoot, 'apps', 'ssh-environment-runtime', 'src', 'main.zig'),
			'pub fn main() void { _ = 2; }\n',
		);
		const changed = await prepareDevelopmentSshEnvironmentRelease(
			{ outputRoot: fixture.outputRoot, sourceRoot: fixture.sourceRoot },
			dependencies,
		);
		assert.notEqual(changed.cohortInputDigest, first.cohortInputDigest);
		assert.notEqual(changed.runtimeVersion, first.runtimeVersion);
		assert.equal(changed.terminalHostVersion, first.terminalHostVersion);
		assert.equal(builds.length, 8);
	} finally {
		await rm(fixture.root, { force: true, recursive: true });
	}
});

test('development release cache fails closed instead of repairing an invalid existing target', async () => {
	const fixture = await createFixture();
	try {
		const builds = [];
		const dependencies = createDependencies(builds);
		const release = await prepareDevelopmentSshEnvironmentRelease(
			{ outputRoot: fixture.outputRoot, sourceRoot: fixture.sourceRoot },
			dependencies,
		);
		await chmod(release.publicKeyFilePath, 0o600);
		await writeFile(release.publicKeyFilePath, 'not a public key\n');
		await assert.rejects(
			prepareDevelopmentSshEnvironmentRelease(
				{ outputRoot: fixture.outputRoot, sourceRoot: fixture.sourceRoot },
				dependencies,
			),
			/valid Ed25519 public key/u,
		);
		assert.equal(builds.length, 4);
	} finally {
		await rm(fixture.root, { force: true, recursive: true });
	}
});

test('concurrent development preparations converge on one fully verified cache winner', async () => {
	const fixture = await createFixture();
	try {
		const builds = [];
		const dependencies = createDependencies(builds);
		const [first, second] = await Promise.all([
			prepareDevelopmentSshEnvironmentRelease(
				{ outputRoot: fixture.outputRoot, sourceRoot: fixture.sourceRoot },
				dependencies,
			),
			prepareDevelopmentSshEnvironmentRelease(
				{ outputRoot: fixture.outputRoot, sourceRoot: fixture.sourceRoot },
				dependencies,
			),
		]);
		assert.deepEqual(second, first);
		assert.equal(builds.length, 8);
		const candidateBuildRoots = new Set(
			builds.map(({ cacheDirectory }) =>
				path.relative(fixture.outputRoot, cacheDirectory).split(path.sep).at(0),
			),
		);
		assert.equal(candidateBuildRoots.size, 2);
		for (const candidateBuildRoot of candidateBuildRoots) {
			assert.match(candidateBuildRoot, /^\.development-release-/u);
		}
		assert.equal(
			(await readdir(fixture.outputRoot)).filter((entry) =>
				entry.startsWith('.development-release-'),
			).length,
			0,
		);
		assert.ok((await readFile(first.trustDescriptorFilePath)).byteLength > 0);
	} finally {
		await rm(fixture.root, { force: true, recursive: true });
	}
});

function createDependencies(builds) {
	return {
		buildComponent: async (options, cacheDirectory) => {
			builds.push({
				cacheDirectory,
				platform: options.platform,
				role: options.role,
				version: options.version,
			});
			const binaryDirectory = path.join(options.installationPrefix, 'bin');
			await mkdir(binaryDirectory, { mode: 0o700, recursive: true });
			await writeFile(
				path.join(binaryDirectory, SSH_ENVIRONMENT_RELEASE_ARTIFACT_FILE_NAMES[options.role]),
				`${options.role}\n${options.platform}\n${options.version}\n`,
				{ mode: 0o700 },
			);
		},
		resolveToolchainIdentity: async () =>
			JSON.stringify({ libDirectory: '/zig/lib', target: 'test-target', version: '0.16.0' }),
	};
}

async function createFixture() {
	const root = await mkdtemp(path.join(tmpdir(), 'orchbee-development-release-test-'));
	const sourceRoot = path.join(root, 'source');
	const outputRoot = path.join(root, 'output');
	for (const role of ['ssh-environment-runtime', 'terminal-host']) {
		const componentRoot = path.join(sourceRoot, 'apps', role);
		await mkdir(path.join(componentRoot, 'src'), { mode: 0o700, recursive: true });
		await Promise.all([
			writeFile(path.join(componentRoot, 'build.zig'), `// ${role} build\n`),
			writeFile(path.join(componentRoot, 'build.zig.zon'), `.{} // ${role}\n`),
			writeFile(path.join(componentRoot, 'src', 'main.zig'), 'pub fn main() void {}\n'),
		]);
	}
	await mkdir(path.join(sourceRoot, 'scripts'), { mode: 0o700, recursive: true });
	await mkdir(path.join(sourceRoot, 'apps', 'desktop', 'scripts'), {
		mode: 0o700,
		recursive: true,
	});
	await Promise.all([
		writeFile(
			path.join(sourceRoot, 'scripts', 'ssh-environment-release.mjs'),
			'// release recipe\n',
		),
		writeFile(
			path.join(
				sourceRoot,
				'apps',
				'desktop',
				'scripts',
				'development-ssh-environment-release.mjs',
			),
			'// development recipe\n',
		),
	]);
	return { outputRoot, root, sourceRoot };
}
