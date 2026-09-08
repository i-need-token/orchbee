import { execFileSync, spawn } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { DESKTOP_AGENT_INTEGRATION_IDS } from '@orchbee/desktop-client-protocol';
import { TerminalLaunchSchema, type TerminalLaunch } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import { createDefaultTerminalProfile } from '../../host';
import {
	resolveAutomaticShellIntegration,
	type AutomaticShellIntegrationConfiguration,
} from '../resolveAutomaticShellIntegration';

const resources = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	'../../../../resources/shell-integration',
);

describe('Desktop shell integration resources', () => {
	it('ships manual, idempotent integrations with bounded Agent launch hints and no command text transport', async () => {
		const [zsh, bash, fish, automaticZsh, automaticBash, automaticFish] = await Promise.all(
			['orchbee.zsh', 'orchbee.bash', 'orchbee.fish']
				.map((name) => readFile(path.join(resources, name), 'utf8'))
				.concat([
					readFile(path.join(resources, 'automatic', 'zsh', '.zshenv'), 'utf8'),
					readFile(path.join(resources, 'automatic', 'bash', 'orchbee.bash'), 'utf8'),
					readFile(
						path.join(
							resources,
							'automatic',
							'fish',
							'fish',
							'vendor_conf.d',
							'orchbee-shell-integration.fish',
						),
						'utf8',
					),
				]),
		);
		for (const source of [zsh, bash, fish]) {
			if (source === undefined) throw new Error('Missing shell integration fixture.');
			expect(source).toContain('133;C');
			expect(source).toContain('133;D;');
			expect(source).toContain('133;A');
			expect(source).toContain('orchbee.agent.v3;%s');
			expect(source).not.toContain(']777;notify');
			expect(source).toContain('file://');
			expect(source).not.toMatch(/cmdline|command[_ -]?text/iu);
			const launchIds = [...source.matchAll(/\b(eyJ[A-Za-z0-9_-]+)\b/gu)]
				.map((match) => match[1])
				.filter((encoded): encoded is string => encoded !== undefined)
				.map((encoded) => JSON.parse(Buffer.from(encoded, 'base64url').toString('utf8')))
				.map((payload: { readonly integrationId: string }) => payload.integrationId)
				.sort();
			expect(launchIds).toEqual([...DESKTOP_AGENT_INTEGRATION_IDS].sort());
		}
		expect(bash).toContain('BASH_VERSINFO[1] < 4');
		expect(bash).toContain('unsupported-bash-version');
		expect(zsh).toContain('__ORCHBEE_SHELL_INTEGRATION_LOADED');
		expect(fish).toContain('__orchbee_shell_integration_loaded');
		expect(automaticZsh).toContain('ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ZDOTDIR');
		expect(automaticBash).toContain('ORCHBEE_SHELL_INTEGRATION_BASH_INJECT');
		expect(automaticBash).not.toContain('ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ENV');
		expect(automaticFish).toContain('ORCHBEE_SHELL_INTEGRATION_ORIGINAL_XDG_DATA_DIRS');
		for (const loader of [automaticZsh, automaticBash, automaticFish]) {
			expect(loader).not.toMatch(/cmdline|command[_ -]?text|history -[rwa]/iu);
		}
	});

	it.runIf(process.platform !== 'win32')(
		'emits only the closed Agent identity for a direct executable Zsh command',
		async () => {
			const temporaryBin = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-command-'));
			try {
				await writeFile(path.join(temporaryBin, 'claude'), '', { mode: 0o755 });
				const script = [
					`source ${shellQuote(path.join(resources, 'orchbee.zsh'))}`,
					`PATH=${shellQuote(temporaryBin)}:$PATH`,
					"__orchbee_preexec 'claude --prompt do-not-transport-this'",
				].join('\n');
				const output = execFileSync('/bin/zsh', ['-f', '-c', script]).toString('utf8');
				expect(output).toContain(
					'\u001B_orchbee.agent.v3;eyJpbnRlZ3JhdGlvbklkIjoiY2xhdWRlLWNvZGUiLCJraW5kIjoibGF1bmNoIiwidmVyc2lvbiI6M30\u001B\\',
				);
				expect(output).not.toContain('do-not-transport-this');
			} finally {
				await rm(temporaryBin, { force: true, recursive: true });
			}
		},
	);

	it.runIf(process.platform !== 'win32')(
		'ignores a shell function that shadows a supported Agent executable',
		async () => {
			const temporaryBin = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-function-'));
			try {
				await writeFile(path.join(temporaryBin, 'codex'), '', { mode: 0o755 });
				const script = [
					`source ${shellQuote(path.join(resources, 'orchbee.zsh'))}`,
					`PATH=${shellQuote(temporaryBin)}:$PATH`,
					'function codex() { :; }',
					"__orchbee_preexec 'codex --prompt do-not-transport-this'",
				].join('\n');
				const output = execFileSync('/bin/zsh', ['-f', '-c', script]).toString('utf8');
				expect(output).toBe('\u001B]133;C\u0007');
			} finally {
				await rm(temporaryBin, { force: true, recursive: true });
			}
		},
	);

	it.runIf(process.platform !== 'win32')(
		'emits one Zsh command boundary with an encoded working directory and exit code',
		() => {
			const script = [
				`source ${shellQuote(path.join(resources, 'orchbee.zsh'))}`,
				'__orchbee_preexec',
				'false',
				'__orchbee_precmd',
			].join('\n');
			const output = execFileSync('/bin/zsh', ['-f', '-c', script]);
			expect(output.toString('hex')).toMatch(
				/^1b5d3133333b43071b5d3133333b443b31071b5d373b66696c653a2f2f.*071b5d3133333b4107$/u,
			);
		},
	);

	it.runIf(process.platform !== 'win32')(
		'loads Zsh automatically while preserving user startup state and clearing one-shot markers',
		async () => {
			const temporaryHome = await mkdtemp(path.join(tmpdir(), 'orchbee-zsh-integration-'));
			const originalZdotdir = path.join(temporaryHome, 'zsh-config');
			try {
				await mkdir(originalZdotdir);
				// Keep distribution startup work outside this user-startup witness.
				await writeFile(
					path.join(originalZdotdir, '.zshenv'),
					'unsetopt globalrcs\ntypeset -gx ORCHBEE_ZSHENV_WITNESS=preserved\n',
					'utf8',
				);
				await writeFile(
					path.join(originalZdotdir, '.zshrc'),
					"typeset -gx ORCHBEE_ZSHRC_WITNESS=preserved\nPS1=''\n",
					'utf8',
				);
				const source = automaticLaunch('/bin/zsh', ['-i']);
				const resolution = resolveAutomaticShellIntegration(
					source,
					process.platform,
					automaticConfiguration({ zdotdir: originalZdotdir }),
				);
				expect(resolution.kind).toBe('injected');
				if (resolution.kind !== 'injected') throw new Error('Expected Zsh injection.');
				const output = await executeInteractiveShell(
					resolution.launch,
					applyLaunchEnvironment(resolution.launch, { HOME: temporaryHome }),
					[
						'printf \'SI_AUTO_ZSH=%s:%s:%s:%s\\n\' "$ORCHBEE_ZSHENV_WITNESS" "$ORCHBEE_ZSHRC_WITNESS" "$ZDOTDIR" "${ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ZDOTDIR-unset}"',
						'exit',
						'',
					].join('\n'),
				);
				const text = output.toString('utf8');
				expect(text).toContain(`SI_AUTO_ZSH=preserved:preserved:${originalZdotdir}:unset`);
				expect(output.toString('hex')).toContain('1b5d3133333b41');
			} finally {
				await rm(temporaryHome, { force: true, recursive: true });
			}
		},
	);

	it.runIf(process.platform !== 'win32')(
		'preserves Bash startup and environment semantics before optional version admission',
		async () => {
			const temporaryHome = await mkdtemp(path.join(tmpdir(), 'orchbee-bash-integration-'));
			try {
				await writeFile(
					path.join(temporaryHome, '.bashrc'),
					"export ORCHBEE_BASHRC_WITNESS=preserved\nPS1=''\n",
					'utf8',
				);
				const source = automaticLaunch('/bin/bash', ['-i']);
				const resolution = resolveAutomaticShellIntegration(
					source,
					process.platform,
					automaticConfiguration(),
				);
				expect(resolution.kind).toBe('injected');
				if (resolution.kind !== 'injected') throw new Error('Expected Bash injection.');
				const output = await executeInteractiveShell(
					resolution.launch,
					applyLaunchEnvironment(resolution.launch, {
						ENV: '/original/posix-env',
						HISTFILE: '/original/history',
						HOME: temporaryHome,
					}),
					[
						'printf \'SI_AUTO_BASH=%s:%s:%s:%s:%s\\n\' "$ORCHBEE_BASHRC_WITNESS" "$ENV" "$HISTFILE" "${ORCHBEE_SHELL_INTEGRATION_BASH_RCFILE-unset}" "${ORCHBEE_SHELL_INTEGRATION_BASH_INJECT-unset}"',
						'exit',
						'',
					].join('\n'),
				);
				expect(output.toString('utf8')).toContain(
					'SI_AUTO_BASH=preserved:/original/posix-env:/original/history:unset:unset',
				);
			} finally {
				await rm(temporaryHome, { force: true, recursive: true });
			}
		},
	);
});

function automaticConfiguration(
	overrides: Partial<AutomaticShellIntegrationConfiguration['inheritedEnvironment']> = {},
): AutomaticShellIntegrationConfiguration {
	return {
		availableShells: ['bash', 'fish', 'zsh'],
		directory: resources,
		inheritedEnvironment: {
			xdgDataDirectories: null,
			zdotdir: null,
			...overrides,
		},
	};
}

function automaticLaunch(executable: string, arguments_: string[]): TerminalLaunch {
	return TerminalLaunchSchema.parse({
		arguments: arguments_,
		cellPixelHeight: 16,
		cellPixelWidth: 8,
		columns: 80,
		configuration: null,
		currentDirectory: null,
		environment: [
			{ name: 'TERM', value: 'xterm-256color' },
			{ name: 'ORCHBEE_SHELL_INTEGRATION_DIR', value: resources },
		],
		executable,
		rows: 24,
		terminalProfile: createDefaultTerminalProfile(),
	});
}

function applyLaunchEnvironment(
	launch: TerminalLaunch,
	overrides: Readonly<Record<string, string>>,
): NodeJS.ProcessEnv {
	const environment: NodeJS.ProcessEnv = { ...process.env, ...overrides };
	for (const entry of launch.environment) {
		if (entry.value === null) delete environment[entry.name];
		else environment[entry.name] = entry.value;
	}
	return environment;
}

async function executeInteractiveShell(
	launch: TerminalLaunch,
	environment: NodeJS.ProcessEnv,
	input: string,
): Promise<Buffer> {
	const child = spawn(launch.executable, launch.arguments, {
		detached: true,
		env: environment,
	});
	const stdout: Buffer[] = [];
	const stderr: Buffer[] = [];
	child.stdout.on('data', (chunk: Buffer) => stdout.push(chunk));
	child.stderr.on('data', (chunk: Buffer) => stderr.push(chunk));

	return await new Promise((resolve, reject) => {
		let timedOut = false;
		const timeout = setTimeout(() => {
			timedOut = true;
			child.kill('SIGKILL');
		}, 3_000);
		child.once('error', (error) => {
			clearTimeout(timeout);
			reject(error);
		});
		child.stdin.once('error', (error) => {
			clearTimeout(timeout);
			child.kill('SIGKILL');
			reject(error);
		});
		child.once('close', (code, signal) => {
			clearTimeout(timeout);
			if (timedOut) {
				reject(new Error('Interactive shell exceeded the 3000 ms witness deadline.'));
				return;
			}
			if (code !== 0) {
				reject(
					new Error(
						`Interactive shell exited with status ${String(code)} (${String(signal)}): ${Buffer.concat(stderr).toString('utf8')}`,
					),
				);
				return;
			}
			resolve(Buffer.concat(stdout));
		});
		child.stdin.end(input);
	});
}

function shellQuote(value: string): string {
	return `'${value.replaceAll("'", `'\\''`)}'`;
}
