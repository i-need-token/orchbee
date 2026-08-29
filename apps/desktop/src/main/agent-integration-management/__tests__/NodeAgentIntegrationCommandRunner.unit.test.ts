import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { describe, expect, it } from 'vitest';

import { NodeAgentIntegrationCommandRunner } from '../NodeAgentIntegrationCommandRunner';

describe('NodeAgentIntegrationCommandRunner', () => {
	it('resolves only bounded executable names to absolute paths', async () => {
		const runner = createRunner();

		expect(await runner.resolveExecutable('sh')).toMatch(/^\//u);
		await expect(runner.resolveExecutable('../sh')).resolves.toBeNull();
		await expect(runner.resolveExecutable('SH')).resolves.toBeNull();
	});

	it('runs only absolute executables with ignored stdin and bounded stdout', async () => {
		const runner = createRunner();

		await expect(runner.run('/bin/sh', ['-c', "printf 'ready'"])).resolves.toEqual({
			kind: 'completed',
			stdout: 'ready',
		});
		await expect(runner.run('sh', ['-c', "printf 'ignored'"])).resolves.toEqual({
			kind: 'failed',
		});
	});

	it('uses one interactive login-shell PATH probe for a restricted desktop environment', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-runner-'));
		try {
			const executableDirectory = path.join(directory, 'bin');
			const probeCountPath = path.join(directory, 'probe-count');
			const shellPath = path.join(directory, 'test-shell');
			await mkdir(executableDirectory);
			await writeFile(
				shellPath,
				[
					'#!/bin/sh',
					"printf 'shell startup noise\\n'",
					'printf \'x\' >> "$ORCHBEE_TEST_PROBE_COUNT_PATH"',
					"probe_command=''",
					'for argument in "$@"; do probe_command="$argument"; done',
					'PATH="$ORCHBEE_TEST_AGENT_SEARCH_PATH"',
					'export PATH',
					'/bin/sh -c "$probe_command"',
					'',
				].join('\n'),
				{ mode: 0o700 },
			);
			await writeFile(
				path.join(executableDirectory, 'orchbee-test-runtime'),
				"#!/bin/sh\nprintf 'resolved-shell-path'\n",
				{ mode: 0o700 },
			);
			const codexPath = path.join(executableDirectory, 'codex');
			await writeFile(codexPath, '#!/usr/bin/env orchbee-test-runtime\n', { mode: 0o700 });
			await writeFile(path.join(executableDirectory, 'opencode'), '#!/bin/sh\n', {
				mode: 0o600,
			});
			const runner = new NodeAgentIntegrationCommandRunner({
				environment: {
					HOME: directory,
					ORCHBEE_TEST_AGENT_SEARCH_PATH: [executableDirectory, '/usr/bin', '/bin'].join(
						path.delimiter,
					),
					ORCHBEE_TEST_PROBE_COUNT_PATH: probeCountPath,
					PATH: ['/usr/bin', '/bin'].join(path.delimiter),
				},
				homeDirectory: directory,
				shellExecutable: shellPath,
			});

			const [resolvedCodexPath, nonExecutablePath] = await Promise.all([
				runner.resolveExecutable('codex'),
				runner.resolveExecutable('opencode'),
			]);

			expect(resolvedCodexPath).toBe(codexPath);
			expect(nonExecutablePath).toBeNull();
			expect(await readFile(probeCountPath, 'utf8')).toBe('x');
			if (resolvedCodexPath === null) return;
			await expect(runner.run(resolvedCodexPath, [])).resolves.toEqual({
				kind: 'completed',
				stdout: 'resolved-shell-path',
			});
		} finally {
			await rm(directory, { force: true, recursive: true });
		}
	});

	it('caches a malformed shell PATH probe as an ordinary search miss', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-runner-'));
		try {
			const probeCountPath = path.join(directory, 'probe-count');
			const shellPath = path.join(directory, 'test-shell');
			await writeFile(
				shellPath,
				[
					'#!/bin/sh',
					'printf \'x\' >> "$ORCHBEE_TEST_PROBE_COUNT_PATH"',
					"printf 'malformed probe output\\n'",
					'',
				].join('\n'),
				{ mode: 0o700 },
			);
			const runner = new NodeAgentIntegrationCommandRunner({
				environment: {
					HOME: directory,
					ORCHBEE_TEST_PROBE_COUNT_PATH: probeCountPath,
					PATH: ['/usr/bin', '/bin'].join(path.delimiter),
				},
				homeDirectory: directory,
				shellExecutable: shellPath,
			});

			await expect(runner.resolveExecutable('codex')).resolves.toBeNull();
			await expect(runner.resolveExecutable('codex')).resolves.toBeNull();
			expect(await readFile(probeCountPath, 'utf8')).toBe('x');
		} finally {
			await rm(directory, { force: true, recursive: true });
		}
	});
});

function createRunner(): NodeAgentIntegrationCommandRunner {
	return new NodeAgentIntegrationCommandRunner({
		environment: process.env,
		homeDirectory: process.cwd(),
		shellExecutable: '/bin/sh',
	});
}
