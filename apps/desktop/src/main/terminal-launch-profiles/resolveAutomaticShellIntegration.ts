import path from 'node:path';

import { TerminalLaunchSchema, type TerminalLaunch } from '@orchbee/terminal-protocol';

export type AutomaticShellIntegrationShell = 'bash' | 'fish' | 'zsh';

export interface AutomaticShellIntegrationConfiguration {
	readonly availableShells: readonly AutomaticShellIntegrationShell[];
	readonly directory: string;
	readonly inheritedEnvironment: {
		readonly xdgDataDirectories: string | null;
		readonly zdotdir: string | null;
	};
}

export type AutomaticShellIntegrationResolution =
	| {
			readonly kind: 'injected';
			readonly launch: TerminalLaunch;
			readonly shell: AutomaticShellIntegrationShell;
	  }
	| {
			readonly kind: 'unchanged';
			readonly launch: TerminalLaunch;
			readonly reason:
				| 'invalid-candidate'
				| 'missing-resources'
				| 'unsafe-resource-path'
				| 'unsupported-arguments'
				| 'unsupported-executable'
				| 'unsupported-platform';
	  };

interface BashArgumentResolution {
	readonly longArguments: string[];
	readonly rcFile: string | null;
	readonly shortArguments: string[];
}

const simpleInteractiveArguments = new Set(['--interactive', '--login', '-i', '-il', '-l', '-li']);
const defaultXdgDataDirectories = '/usr/local/share:/usr/share';

/** Refine one complete profile-selected launch without performing process or filesystem effects. */
export function resolveAutomaticShellIntegration(
	launch: TerminalLaunch,
	platform: NodeJS.Platform,
	configuration: AutomaticShellIntegrationConfiguration | null,
): AutomaticShellIntegrationResolution {
	if (platform === 'win32') return unchanged(launch, 'unsupported-platform');
	const shell = shellFromExecutable(launch.executable);
	if (shell === null) return unchanged(launch, 'unsupported-executable');
	if (configuration === null || !configuration.availableShells.includes(shell)) {
		return unchanged(launch, 'missing-resources');
	}
	if (!path.posix.isAbsolute(configuration.directory)) {
		return unchanged(launch, 'invalid-candidate');
	}

	let candidate: TerminalLaunch;
	if (shell === 'zsh') {
		if (!hasSimpleInteractiveArguments(launch.arguments)) {
			return unchanged(launch, 'unsupported-arguments');
		}
		candidate = withEnvironment(launch, [
			{
				name: 'ORCHBEE_SHELL_INTEGRATION_ORIGINAL_ZDOTDIR',
				value: configuration.inheritedEnvironment.zdotdir,
			},
			{
				name: 'ZDOTDIR',
				value: path.posix.join(configuration.directory, 'automatic', 'zsh'),
			},
		]);
	} else if (shell === 'fish') {
		if (!hasSimpleInteractiveArguments(launch.arguments)) {
			return unchanged(launch, 'unsupported-arguments');
		}
		const automaticDirectory = path.posix.join(configuration.directory, 'automatic', 'fish');
		if (automaticDirectory.includes(':')) {
			return unchanged(launch, 'unsafe-resource-path');
		}
		const inherited = configuration.inheritedEnvironment.xdgDataDirectories;
		const effectiveInherited =
			inherited === null || inherited.length === 0 ? defaultXdgDataDirectories : inherited;
		candidate = withEnvironment(launch, [
			{
				name: 'ORCHBEE_SHELL_INTEGRATION_ORIGINAL_XDG_DATA_DIRS',
				value: inherited,
			},
			{ name: 'XDG_DATA_DIRS', value: `${automaticDirectory}:${effectiveInherited}` },
		]);
	} else {
		const bashArguments = resolveBashArguments(launch.arguments);
		if (bashArguments === null) return unchanged(launch, 'unsupported-arguments');
		const automaticLoader = path.posix.join(
			configuration.directory,
			'automatic',
			'bash',
			'orchbee.bash',
		);
		candidate = withEnvironment(
			{
				...launch,
				arguments: [
					...bashArguments.longArguments,
					'--rcfile',
					automaticLoader,
					...bashArguments.shortArguments,
				],
			},
			[
				{ name: 'ORCHBEE_SHELL_INTEGRATION_BASH_INJECT', value: 'enabled' },
				{ name: 'ORCHBEE_SHELL_INTEGRATION_BASH_RCFILE', value: bashArguments.rcFile },
			],
		);
	}

	const parsed = TerminalLaunchSchema.safeParse(candidate);
	return parsed.success
		? { kind: 'injected', launch: parsed.data, shell }
		: unchanged(launch, 'invalid-candidate');
}

function shellFromExecutable(executable: string): AutomaticShellIntegrationShell | null {
	const basename = path.posix.basename(executable);
	return basename === 'bash' || basename === 'fish' || basename === 'zsh' ? basename : null;
}

function hasSimpleInteractiveArguments(arguments_: readonly string[]): boolean {
	return arguments_.every((argument) => simpleInteractiveArguments.has(argument));
}

function resolveBashArguments(arguments_: readonly string[]): BashArgumentResolution | null {
	const longArguments: string[] = [];
	let rcFile: string | null = null;
	const shortArguments: string[] = [];
	for (let index = 0; index < arguments_.length; index += 1) {
		const argument = arguments_[index];
		if (argument === undefined) return null;
		if (argument === '--noprofile') {
			longArguments.push(argument);
			continue;
		}
		if (argument === '--rcfile' || argument === '--init-file') {
			const value = arguments_[index + 1];
			if (value === undefined || rcFile !== null) return null;
			rcFile = value;
			index += 1;
			continue;
		}
		if (argument === '--interactive') {
			longArguments.push(argument);
			continue;
		}
		if (argument === '-i') {
			shortArguments.push(argument);
			continue;
		}
		return null;
	}
	return { longArguments, rcFile, shortArguments };
}

function withEnvironment(
	launch: TerminalLaunch,
	replacements: readonly { readonly name: string; readonly value: string | null }[],
): TerminalLaunch {
	const replacementByName = new Map(replacements.map((entry) => [entry.name, entry]));
	const existingNames = new Set(launch.environment.map((entry) => entry.name));
	return {
		...launch,
		environment: [
			...launch.environment.map((entry) => replacementByName.get(entry.name) ?? entry),
			...replacements.filter((entry) => !existingNames.has(entry.name)),
		],
	};
}

function unchanged(
	launch: TerminalLaunch,
	reason: Extract<AutomaticShellIntegrationResolution, { readonly kind: 'unchanged' }>['reason'],
): AutomaticShellIntegrationResolution {
	return { kind: 'unchanged', launch, reason };
}
