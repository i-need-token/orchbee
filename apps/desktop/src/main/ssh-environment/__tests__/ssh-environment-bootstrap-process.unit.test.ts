import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { chmod, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentReleaseCohortDigestSchema,
	SshEnvironmentRuntimeVersionSchema,
	SshEnvironmentTerminalHostVersionSchema,
	type SshEnvironmentPlatform,
} from '@orchbee/ssh-environment-protocol';
import { afterEach, describe, expect, it } from 'vitest';

import { createSshEnvironmentRemoteBootstrap } from '../createSshEnvironmentRemoteBootstrap';
import { SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES } from '../ssh-environment-bootstrap-negotiation';
import type { VerifiedSshEnvironmentReleaseCatalog } from '../verify-ssh-environment-release';

const temporaryRoots = new Set<string>();
const runtimeVersion = SshEnvironmentRuntimeVersionSchema.parse('0.2.0');
const terminalHostVersion = SshEnvironmentTerminalHostVersionSchema.parse('0.1.0');
const runtimeInstanceId = '1'.repeat(32);
const runtimeArtifactBytes = Buffer.from('#!/bin/sh\nexit 0\n');
const terminalHostArtifactBytes = Buffer.from('#!/bin/sh\nexit 64\n');
const runtimeArtifactDigest = digest(runtimeArtifactBytes);
const terminalHostArtifactDigest = digest(terminalHostArtifactBytes);
const nativePlatform = resolveNativePlatform();

afterEach(async () => {
	await Promise.all([...temporaryRoots].map((root) => rm(root, { force: true, recursive: true })));
	temporaryRoots.clear();
});

describe.skipIf(nativePlatform === null)('SSH Environment bootstrap cohort process', () => {
	it('commits both uploads, reuses exact bytes, and blocks mutation or symlink substitution', async () => {
		if (nativePlatform === null) return;
		const root = await createBootstrapRoot('orchbee-bootstrap-process-');
		const bootstrap = createSshEnvironmentRemoteBootstrap({
			catalog: createCatalog(),
			runtimeInstanceId,
		});
		const environment = createEnvironment(root);

		const first = startBootstrap(bootstrap.command, environment);
		await expect(first.nextLine()).resolves.toBe(
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform]['ssh-environment-runtime'].upload,
		);
		const hostStatus = first.nextLine();
		first.child.stdin.write(runtimeArtifactBytes);
		await expect(hostStatus).resolves.toBe(
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform]['terminal-host'].upload,
		);
		const commit = first.nextLine();
		first.child.stdin.end(terminalHostArtifactBytes);
		await expect(commit).resolves.toBe(
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform].committed,
		);
		await expect(first.exit).resolves.toBe(75);

		const runtimeInstalledPath = runtimePath(root);
		const terminalHostInstalledPath = terminalHostPath(root);
		await expect(readFile(runtimeInstalledPath)).resolves.toEqual(runtimeArtifactBytes);
		await expect(readFile(terminalHostInstalledPath)).resolves.toEqual(terminalHostArtifactBytes);

		const second = startBootstrap(bootstrap.command, environment);
		await expect(second.nextLine()).resolves.toBe(
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform]['ssh-environment-runtime'].present,
		);
		await expect(second.nextLine()).resolves.toBe(
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform]['terminal-host'].present,
		);
		await expect(second.nextLine()).resolves.toBe(
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform].committed,
		);
		second.child.stdin.end();
		await expect(second.exit).resolves.toBe(75);

		await writeFile(runtimeInstalledPath, Buffer.alloc(runtimeArtifactBytes.byteLength, 0), {
			mode: 0o700,
		});
		const mutated = startBootstrap(bootstrap.command, environment);
		mutated.child.stdin.end();
		await expect(mutated.exit).resolves.toBe(78);
		expect(mutated.stdout()).toBe('');

		await writeFile(runtimeInstalledPath, runtimeArtifactBytes, { mode: 0o700 });
		await rm(terminalHostInstalledPath);
		await symlink(path.join(root, 'outside-terminal-host'), terminalHostInstalledPath);
		const symlinked = startBootstrap(bootstrap.command, environment);
		symlinked.child.stdin.end();
		await expect(symlinked.exit).resolves.toBe(78);
		expect(symlinked.stdout()).toContain(
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform]['ssh-environment-runtime'].present,
		);
		expect(symlinked.stdout()).not.toContain(
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform].committed,
		);
	}, 15_000);

	it('converges concurrent cohort uploads on no-clobber installed inodes', async () => {
		if (nativePlatform === null) return;
		const root = await createBootstrapRoot('orchbee-bootstrap-race-');
		const bootstrap = createSshEnvironmentRemoteBootstrap({
			catalog: createCatalog(),
			runtimeInstanceId,
		});
		const environment = createEnvironment(root);
		const first = startBootstrap(bootstrap.command, environment);
		const second = startBootstrap(bootstrap.command, environment);
		await expect(Promise.all([first.nextLine(), second.nextLine()])).resolves.toEqual([
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform]['ssh-environment-runtime'].upload,
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform]['ssh-environment-runtime'].upload,
		]);
		const hostStatuses = Promise.all([first.nextLine(), second.nextLine()]);
		first.child.stdin.write(runtimeArtifactBytes);
		second.child.stdin.write(runtimeArtifactBytes);
		await expect(hostStatuses).resolves.toEqual([
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform]['terminal-host'].upload,
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform]['terminal-host'].upload,
		]);
		const commits = Promise.all([first.nextLine(), second.nextLine()]);
		first.child.stdin.end(terminalHostArtifactBytes);
		second.child.stdin.end(terminalHostArtifactBytes);
		await expect(commits).resolves.toEqual([
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform].committed,
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[nativePlatform].committed,
		]);
		await expect(Promise.all([first.exit, second.exit])).resolves.toEqual([75, 75]);
		await expect(readFile(runtimePath(root))).resolves.toEqual(runtimeArtifactBytes);
		await expect(readFile(terminalHostPath(root))).resolves.toEqual(terminalHostArtifactBytes);
	}, 15_000);
});

function createCatalog(): VerifiedSshEnvironmentReleaseCatalog {
	const descriptor = (
		platform: SshEnvironmentPlatform,
		buildDigest: ReturnType<typeof digest>,
		sizeBytes: number,
	) => ({ buildDigest, platform, sizeBytes });
	return {
		components: {
			'ssh-environment-runtime': {
				artifacts: {
					'darwin-arm64': descriptor(
						'darwin-arm64',
						runtimeArtifactDigest,
						runtimeArtifactBytes.byteLength,
					),
					'linux-x64': descriptor(
						'linux-x64',
						runtimeArtifactDigest,
						runtimeArtifactBytes.byteLength,
					),
				},
				role: 'ssh-environment-runtime',
				version: runtimeVersion,
			},
			'terminal-host': {
				artifacts: {
					'darwin-arm64': descriptor(
						'darwin-arm64',
						terminalHostArtifactDigest,
						terminalHostArtifactBytes.byteLength,
					),
					'linux-x64': descriptor(
						'linux-x64',
						terminalHostArtifactDigest,
						terminalHostArtifactBytes.byteLength,
					),
				},
				role: 'terminal-host',
				version: terminalHostVersion,
			},
		},
		releaseCohortDigest: SshEnvironmentReleaseCohortDigestSchema.parse('a'.repeat(64)),
	};
}

async function createBootstrapRoot(prefix: string): Promise<string> {
	const root = await mkdtemp(path.join(tmpdir(), prefix));
	temporaryRoots.add(root);
	const fakeBin = path.join(root, 'bin');
	await mkdir(fakeBin);
	const fakeSleep = path.join(fakeBin, 'sleep');
	await writeFile(fakeSleep, '#!/bin/sh\nexit 0\n', { mode: 0o700 });
	await chmod(fakeSleep, 0o700);
	return root;
}

function createEnvironment(root: string): NodeJS.ProcessEnv {
	return {
		...process.env,
		HOME: root,
		PATH: `${path.join(root, 'bin')}:${process.env['PATH'] ?? '/usr/bin:/bin'}`,
	};
}

function runtimePath(root: string): string {
	return path.join(
		root,
		'.orchbee',
		'ssh-runtime',
		'versions',
		runtimeVersion,
		'orchbee-ssh-environment-runtime',
	);
}

function terminalHostPath(root: string): string {
	return path.join(
		root,
		'.orchbee',
		'terminal-host',
		'versions',
		terminalHostVersion,
		'orchbee-terminal-host',
	);
}

function startBootstrap(command: string, environment: NodeJS.ProcessEnv) {
	const child = spawn('/bin/sh', ['-c', command], {
		env: environment,
		stdio: ['pipe', 'pipe', 'pipe'],
	});
	let output = '';
	let pending = '';
	const lines: string[] = [];
	const waiters: Array<(line: string) => void> = [];
	child.stdout.on('data', (chunk: Buffer) => {
		const text = chunk.toString('ascii');
		output += text;
		pending += text;
		while (true) {
			const newline = pending.indexOf('\n');
			if (newline < 0) break;
			const line = pending.slice(0, newline);
			pending = pending.slice(newline + 1);
			const waiter = waiters.shift();
			if (waiter === undefined) lines.push(line);
			else waiter(line);
		}
	});
	const exit = new Promise<number | null>((resolve, reject) => {
		child.once('error', reject);
		child.once('close', resolve);
	});
	const nextLine = (): Promise<string> => {
		const line = lines.shift();
		if (line !== undefined) return Promise.resolve(line);
		return new Promise((resolve, reject) => {
			const timeout = setTimeout(() => reject(new Error('Bootstrap status timed out.')), 5_000);
			waiters.push((value) => {
				clearTimeout(timeout);
				resolve(value);
			});
		});
	};
	return { child, exit, nextLine, stdout: () => output };
}

function digest(bytes: Uint8Array) {
	return SshEnvironmentBuildDigestSchema.parse(createHash('sha256').update(bytes).digest('hex'));
}

function resolveNativePlatform(): SshEnvironmentPlatform | null {
	if (process.platform === 'darwin' && process.arch === 'arm64') return 'darwin-arm64';
	if (process.platform === 'linux' && process.arch === 'x64') return 'linux-x64';
	return null;
}
