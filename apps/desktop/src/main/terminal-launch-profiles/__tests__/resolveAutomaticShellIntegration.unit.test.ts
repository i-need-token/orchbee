import { TerminalLaunchSchema, type TerminalLaunch } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import { createDefaultTerminalProfile } from '../../host';
import {
	resolveAutomaticShellIntegration,
	type AutomaticShellIntegrationConfiguration,
} from '../resolveAutomaticShellIntegration';

const integrationDirectory = '/Applications/Orchbee.app/Contents/Resources/shell-integration';

function configuration(
	overrides: Partial<AutomaticShellIntegrationConfiguration> = {},
): AutomaticShellIntegrationConfiguration {
	return {
		availableShells: ['bash', 'fish', 'zsh'],
		directory: integrationDirectory,
		inheritedEnvironment: {
			xdgDataDirectories: null,
			zdotdir: null,
		},
		...overrides,
	};
}

function launch(
	executable: string,
	arguments_: readonly string[] = [],
	environment: TerminalLaunch['environment'] = [
		{ name: 'TERM', value: 'xterm-256color' },
		{ name: 'ORCHBEE_SHELL_INTEGRATION_DIR', value: integrationDirectory },
	],
): TerminalLaunch {
	return TerminalLaunchSchema.parse({
		arguments: arguments_,
		cellPixelHeight: 16,
		cellPixelWidth: 8,
		columns: 80,
		configuration: null,
		currentDirectory: '/Users/orchbee/project',
		environment,
		executable,
		rows: 24,
		terminalProfile: createDefaultTerminalProfile(),
	});
}

function environmentValue(result: TerminalLaunch, name: string): string | null | undefined {
	return result.environment.find((entry) => entry.name === name)?.value;
}

describe('resolveAutomaticShellIntegration', () => {
	it('refines Zsh after profile selection while preserving every unrelated launch field', () => {
		const source = launch('/opt/homebrew/bin/zsh', ['-li']);
		const result = resolveAutomaticShellIntegration(source, 'darwin', configuration());

		expect(result).toMatchObject({ kind: 'injected', shell: 'zsh' });
		if (result.kind !== 'injected') throw new Error('Expected Zsh injection.');
		expect(result.launch).toMatchObject({
			arguments: ['-li'],
			cellPixelHeight: source.cellPixelHeight,
			cellPixelWidth: source.cellPixelWidth,
			columns: source.columns,
			configuration: source.configuration,
			currentDirectory: source.currentDirectory,
			executable: source.executable,
			rows: source.rows,
			terminalProfile: source.terminalProfile,
		});
		expect(environmentValue(result.launch, 'ZDOTDIR')).toBe(
			`${integrationDirectory}/automatic/zsh`,
		);
		expect(
			environmentValue(result.launch, 'ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ZDOTDIR'),
		).toBeNull();
		expect(result.launch.environment.slice(0, source.environment.length)).toEqual(
			source.environment,
		);
	});

	it.each([
		['unset', null],
		['empty', ''],
		['custom', '/Users/orchbee/.config/zsh'],
	] as const)('carries the exact %s ZDOTDIR state', (_label, zdotdir) => {
		const source = launch('/bin/zsh');
		const result = resolveAutomaticShellIntegration(
			source,
			'linux',
			configuration({
				inheritedEnvironment: {
					xdgDataDirectories: null,
					zdotdir,
				},
			}),
		);

		expect(result.kind).toBe('injected');
		if (result.kind !== 'injected') throw new Error('Expected Zsh injection.');
		expect(environmentValue(result.launch, 'ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ZDOTDIR')).toBe(
			zdotdir,
		);
	});

	it.each(
		[[], ['-i'], ['--interactive'], ['-l'], ['--login'], ['-il'], ['-li'], ['-i', '-l']].map(
			(arguments_) => ({ arguments_ }),
		),
	)('admits the bounded Zsh and Fish interactive argument form %j', ({ arguments_ }) => {
		for (const executable of ['/bin/zsh', '/opt/bin/fish']) {
			expect(
				resolveAutomaticShellIntegration(launch(executable, arguments_), 'linux', configuration())
					.kind,
			).toBe('injected');
		}
	});

	it.each([['-c', 'echo changed'], ['script.zsh'], ['-f'], ['--no-rcs'], ['-ix']])(
		'leaves an unsupported Zsh or Fish argument vector %j unchanged',
		(...arguments_) => {
			for (const executable of ['/bin/zsh', '/opt/bin/fish']) {
				const source = launch(executable, arguments_);
				expect(resolveAutomaticShellIntegration(source, 'linux', configuration())).toEqual({
					kind: 'unchanged',
					launch: source,
					reason: 'unsupported-arguments',
				});
			}
		},
	);

	it('prepends the Fish vendor directory and preserves the exact inherited value', () => {
		const source = launch('/opt/homebrew/bin/fish', ['--login']);
		const result = resolveAutomaticShellIntegration(
			source,
			'darwin',
			configuration({
				inheritedEnvironment: {
					xdgDataDirectories: '/opt/share:/usr/share',
					zdotdir: null,
				},
			}),
		);

		expect(result).toMatchObject({ kind: 'injected', shell: 'fish' });
		if (result.kind !== 'injected') throw new Error('Expected Fish injection.');
		expect(environmentValue(result.launch, 'XDG_DATA_DIRS')).toBe(
			`${integrationDirectory}/automatic/fish:/opt/share:/usr/share`,
		);
		expect(
			environmentValue(result.launch, 'ORCHBEE_SHELL_INTEGRATION_ORIGINAL_XDG_DATA_DIRS'),
		).toBe('/opt/share:/usr/share');
	});

	it.each([
		[null, '/usr/local/share:/usr/share'],
		['', '/usr/local/share:/usr/share'],
	] as const)('keeps Fish defaults available for an inherited %j value', (inherited, fallback) => {
		const result = resolveAutomaticShellIntegration(
			launch('/usr/bin/fish'),
			'linux',
			configuration({
				inheritedEnvironment: {
					xdgDataDirectories: inherited,
					zdotdir: null,
				},
			}),
		);

		expect(result.kind).toBe('injected');
		if (result.kind !== 'injected') throw new Error('Expected Fish injection.');
		expect(environmentValue(result.launch, 'XDG_DATA_DIRS')).toBe(
			`${integrationDirectory}/automatic/fish:${fallback}`,
		);
		expect(
			environmentValue(result.launch, 'ORCHBEE_SHELL_INTEGRATION_ORIGINAL_XDG_DATA_DIRS'),
		).toBe(inherited);
	});

	it('intercepts only the Bash startup flags owned by the automatic loader', () => {
		const source = launch('/opt/bin/bash', [
			'--noprofile',
			'--rcfile',
			'/Users/orchbee/custom bashrc',
			'-i',
		]);
		const result = resolveAutomaticShellIntegration(source, 'linux', configuration());

		expect(result).toMatchObject({ kind: 'injected', shell: 'bash' });
		if (result.kind !== 'injected') throw new Error('Expected Bash injection.');
		expect(result.launch.arguments).toEqual([
			'--noprofile',
			'--rcfile',
			`${integrationDirectory}/automatic/bash/orchbee.bash`,
			'-i',
		]);
		expect(environmentValue(result.launch, 'ORCHBEE_SHELL_INTEGRATION_BASH_INJECT')).toBe(
			'enabled',
		);
		expect(environmentValue(result.launch, 'ORCHBEE_SHELL_INTEGRATION_BASH_RCFILE')).toBe(
			'/Users/orchbee/custom bashrc',
		);
	});

	it('leaves Bash ENV and HISTFILE launch deltas untouched', () => {
		const source = launch(
			'/bin/bash',
			['-i'],
			[
				{ name: 'TERM', value: 'xterm-256color' },
				{ name: 'ENV', value: '/Users/orchbee/.posix-env' },
				{ name: 'HISTFILE', value: '' },
			],
		);
		const result = resolveAutomaticShellIntegration(source, 'darwin', configuration());

		expect(result.kind).toBe('injected');
		if (result.kind !== 'injected') throw new Error('Expected Bash injection.');
		expect(environmentValue(result.launch, 'ENV')).toBe('/Users/orchbee/.posix-env');
		expect(environmentValue(result.launch, 'HISTFILE')).toBe('');
		expect(
			environmentValue(result.launch, 'ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ENV'),
		).toBeUndefined();
	});

	it.each([
		['command', ['-c', 'echo no']],
		['stdin', ['-s']],
		['login', ['--login']],
		['short login', ['-l']],
		['no rc', ['--norc']],
		['posix', ['--posix']],
		['script', ['script.bash']],
		['missing rcfile', ['--rcfile']],
		['duplicate rcfile', ['--rcfile', 'one', '--init-file', 'two']],
		['unknown long option', ['--debugger']],
		['unsupported short option', ['-x']],
	] as const)('leaves the unsupported Bash %s launch unchanged', (_label, arguments_) => {
		const source = launch('/bin/bash', arguments_);
		expect(resolveAutomaticShellIntegration(source, 'linux', configuration())).toEqual({
			kind: 'unchanged',
			launch: source,
			reason: 'unsupported-arguments',
		});
	});

	it('uses stable no-op reasons without partially changing the launch', () => {
		const windows = launch('C:\\Windows\\System32\\bash.exe');
		expect(resolveAutomaticShellIntegration(windows, 'win32', configuration())).toEqual({
			kind: 'unchanged',
			launch: windows,
			reason: 'unsupported-platform',
		});

		const unsupported = launch('/bin/sh');
		expect(resolveAutomaticShellIntegration(unsupported, 'linux', configuration())).toEqual({
			kind: 'unchanged',
			launch: unsupported,
			reason: 'unsupported-executable',
		});

		const missing = launch('/bin/zsh');
		expect(resolveAutomaticShellIntegration(missing, 'linux', null)).toEqual({
			kind: 'unchanged',
			launch: missing,
			reason: 'missing-resources',
		});
		expect(
			resolveAutomaticShellIntegration(
				missing,
				'linux',
				configuration({ availableShells: ['bash', 'fish'] }),
			),
		).toEqual({ kind: 'unchanged', launch: missing, reason: 'missing-resources' });
	});

	it('rejects a Fish path-list ambiguity without changing the launch', () => {
		const source = launch('/bin/fish');
		expect(
			resolveAutomaticShellIntegration(
				source,
				'linux',
				configuration({ directory: '/Applications/Orchbee:Preview/resources' }),
			),
		).toEqual({ kind: 'unchanged', launch: source, reason: 'unsafe-resource-path' });
	});

	it('rejects an over-limit inherited value as one unchanged candidate', () => {
		const source = launch('/bin/zsh');
		const result = resolveAutomaticShellIntegration(
			source,
			'linux',
			configuration({
				inheritedEnvironment: {
					xdgDataDirectories: null,
					zdotdir: 'x'.repeat(17 * 1024 * 1024),
				},
			}),
		);

		expect(result).toEqual({ kind: 'unchanged', launch: source, reason: 'invalid-candidate' });
	});
});
