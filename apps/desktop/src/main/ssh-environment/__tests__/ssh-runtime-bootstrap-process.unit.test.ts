import { createHash } from 'node:crypto';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { chmod, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentRuntimeVersionSchema,
	type SshEnvironmentPlatform,
} from '@orchbee/ssh-environment-protocol';
import { afterEach, describe, expect, it } from 'vitest';

import { createSshRuntimeRemoteBootstrap } from '../createSshRuntimeRemoteBootstrap';
import { SSH_RUNTIME_BOOTSTRAP_STATUS_LINES } from '../ssh-runtime-bootstrap-negotiation';
import type { VerifiedSshRuntimeReleaseCatalog } from '../verify-ssh-runtime-artifact';

const temporaryRoots = new Set<string>();
const runtimeVersion = SshEnvironmentRuntimeVersionSchema.parse('0.1.0');
const runtimeInstanceId = '1'.repeat(32);
const artifactBytes = Buffer.from('#!/bin/sh\nexit 0\n');
const artifactDigest = SshEnvironmentBuildDigestSchema.parse(
	createHash('sha256').update(artifactBytes).digest('hex'),
);
const nativePlatform = resolveNativePlatform();

afterEach(async () => {
	await Promise.all([...temporaryRoots].map((root) => rm(root, { force: true, recursive: true })));
	temporaryRoots.clear();
});

describe.skipIf(nativePlatform === null)('SSH Runtime bootstrap process', () => {
	it('uploads once, reuses exact installed bytes without a second upload, and blocks mutation', async () => {
		if (nativePlatform === null) return;
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-bootstrap-process-'));
		temporaryRoots.add(root);
		const fakeBin = path.join(root, 'bin');
		await mkdir(fakeBin);
		const fakeSleep = path.join(fakeBin, 'sleep');
		await writeFile(fakeSleep, '#!/bin/sh\nexit 0\n', { mode: 0o700 });
		await chmod(fakeSleep, 0o700);
		const bootstrap = createSshRuntimeRemoteBootstrap({
			catalog: createCatalog(),
			runtimeInstanceId,
		});
		const environment = {
			...process.env,
			HOME: root,
			PATH: `${fakeBin}:${process.env['PATH'] ?? '/usr/bin:/bin'}`,
		};

		const first = startBootstrap(bootstrap.command, environment);
		await expect(readStatusLine(first.child)).resolves.toBe(
			SSH_RUNTIME_BOOTSTRAP_STATUS_LINES[nativePlatform].upload,
		);
		first.child.stdin.end(artifactBytes);
		await expect(first.exit).resolves.toBe(75);

		const installedPath = path.join(
			root,
			'.orchbee',
			'ssh-runtime',
			'versions',
			runtimeVersion,
			'orchbee-ssh-environment-runtime',
		);
		await expect(readFile(installedPath)).resolves.toEqual(artifactBytes);

		const second = startBootstrap(bootstrap.command, environment);
		await expect(readStatusLine(second.child)).resolves.toBe(
			SSH_RUNTIME_BOOTSTRAP_STATUS_LINES[nativePlatform].present,
		);
		second.child.stdin.end();
		await expect(second.exit).resolves.toBe(75);

		await writeFile(installedPath, Buffer.alloc(artifactBytes.byteLength, 0), { mode: 0o700 });
		const mutated = startBootstrap(bootstrap.command, environment);
		mutated.child.stdin.end();
		await expect(mutated.exit).resolves.toBe(78);
		expect(mutated.stdout()).toBe('');

		await rm(installedPath);
		await symlink(path.join(root, 'outside-runtime'), installedPath);
		const symlinked = startBootstrap(bootstrap.command, environment);
		symlinked.child.stdin.end();
		await expect(symlinked.exit).resolves.toBe(78);
		expect(symlinked.stdout()).toBe('');
	});

	it('converges two concurrent upload requests on one no-clobber installed inode', async () => {
		if (nativePlatform === null) return;
		const root = await mkdtemp(path.join(tmpdir(), 'orchbee-bootstrap-race-'));
		temporaryRoots.add(root);
		const fakeBin = path.join(root, 'bin');
		await mkdir(fakeBin);
		const fakeSleep = path.join(fakeBin, 'sleep');
		await writeFile(fakeSleep, '#!/bin/sh\nexit 0\n', { mode: 0o700 });
		await chmod(fakeSleep, 0o700);
		const bootstrap = createSshRuntimeRemoteBootstrap({
			catalog: createCatalog(),
			runtimeInstanceId,
		});
		const environment = {
			...process.env,
			HOME: root,
			PATH: `${fakeBin}:${process.env['PATH'] ?? '/usr/bin:/bin'}`,
		};
		const first = startBootstrap(bootstrap.command, environment);
		const second = startBootstrap(bootstrap.command, environment);
		await expect(
			Promise.all([readStatusLine(first.child), readStatusLine(second.child)]),
		).resolves.toEqual([
			SSH_RUNTIME_BOOTSTRAP_STATUS_LINES[nativePlatform].upload,
			SSH_RUNTIME_BOOTSTRAP_STATUS_LINES[nativePlatform].upload,
		]);
		first.child.stdin.end(artifactBytes);
		second.child.stdin.end(artifactBytes);
		await expect(Promise.all([first.exit, second.exit])).resolves.toEqual([75, 75]);
		await expect(
			readFile(
				path.join(
					root,
					'.orchbee',
					'ssh-runtime',
					'versions',
					runtimeVersion,
					'orchbee-ssh-environment-runtime',
				),
			),
		).resolves.toEqual(artifactBytes);
	});
});

function createCatalog(): VerifiedSshRuntimeReleaseCatalog {
	const descriptor = (platform: SshEnvironmentPlatform) => ({
		buildDigest: artifactDigest,
		platform,
		runtimeVersion,
		sizeBytes: artifactBytes.byteLength,
	});
	return {
		artifacts: {
			'darwin-arm64': descriptor('darwin-arm64'),
			'linux-x64': descriptor('linux-x64'),
		},
		runtimeVersion,
	};
}

function startBootstrap(command: string, environment: NodeJS.ProcessEnv) {
	const child = spawn('/bin/sh', ['-c', command], {
		env: environment,
		stdio: ['pipe', 'pipe', 'pipe'],
	});
	let output = '';
	child.stdout.on('data', (chunk: Buffer) => {
		output += chunk.toString('utf8');
	});
	const exit = new Promise<number | null>((resolve, reject) => {
		child.once('error', reject);
		child.once('close', resolve);
	});
	return { child, exit, stdout: () => output };
}

async function readStatusLine(child: ChildProcessWithoutNullStreams): Promise<string> {
	let bytes = Buffer.alloc(0);
	return new Promise((resolve, reject) => {
		const timeout = setTimeout(() => reject(new Error('Bootstrap status timed out.')), 5_000);
		const onData = (chunk: Buffer): void => {
			bytes = Buffer.concat([bytes, chunk]);
			const newline = bytes.indexOf(0x0a);
			if (newline < 0) return;
			clearTimeout(timeout);
			child.stdout.off('data', onData);
			resolve(bytes.subarray(0, newline).toString('ascii'));
		};
		child.stdout.on('data', onData);
		child.once('error', (error) => {
			clearTimeout(timeout);
			reject(error);
		});
	});
}

function resolveNativePlatform(): SshEnvironmentPlatform | null {
	if (process.platform === 'darwin' && process.arch === 'arm64') return 'darwin-arm64';
	if (process.platform === 'linux' && process.arch === 'x64') return 'linux-x64';
	return null;
}
