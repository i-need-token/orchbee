import { constants } from 'node:fs';
import { access, readFile, realpath, stat } from 'node:fs/promises';
import path from 'node:path';

import type { DesktopInstalledShell } from '@orchbee/desktop-client-protocol';

/** Shells Homebrew installs that `/etc/shells` often lacks, in the order they are listed. */
const HOMEBREW_SHELLS = ['zsh', 'bash', 'fish', 'nu', 'pwsh', 'elvish', 'xonsh'] as const;
const HOMEBREW_FOLDERS = ['/opt/homebrew/bin', '/usr/local/bin'] as const;
const MAXIMUM_SHELLS = 64;

async function isFile(candidate: string): Promise<boolean> {
	try {
		return (await stat(candidate)).isFile();
	} catch {
		return false;
	}
}

async function isExecutableFile(candidate: string): Promise<boolean> {
	if (!(await isFile(candidate))) return false;
	try {
		await access(candidate, constants.X_OK);
		return true;
	} catch {
		return false;
	}
}

async function firstFile(candidates: readonly (string | null)[]): Promise<string | null> {
	for (const candidate of candidates) {
		if (candidate !== null && path.win32.isAbsolute(candidate) && (await isFile(candidate))) {
			return candidate;
		}
	}
	return null;
}

async function windowsShells(environment: NodeJS.ProcessEnv): Promise<DesktopInstalledShell[]> {
	const search = (environment['PATH'] ?? environment['Path'] ?? '')
		.split(';')
		.filter((folder) => path.win32.isAbsolute(folder));
	const onPath = (file: string): (string | null)[] =>
		search.map((folder) => path.win32.join(folder, file));
	const under = (variable: string, relative: string): string | null => {
		const root = environment[variable];
		return root === undefined || root === '' ? null : path.win32.join(root, relative);
	};
	// Git for Windows puts `git.exe` in `cmd`, beside the `bin` folder that holds its bash.
	const gitOnPath = await firstFile(onPath('git.exe'));
	const gitBash =
		gitOnPath === null
			? null
			: path.win32.join(path.win32.dirname(path.win32.dirname(gitOnPath)), 'bin', 'bash.exe');
	const shells: [DesktopInstalledShell['kind'], string | null, readonly string[]][] = [
		[
			'powershell',
			await firstFile([...onPath('pwsh.exe'), under('ProgramFiles', 'PowerShell\\7\\pwsh.exe')]),
			[],
		],
		[
			'windows-powershell',
			await firstFile([under('SystemRoot', 'System32\\WindowsPowerShell\\v1.0\\powershell.exe')]),
			[],
		],
		['command-prompt', await firstFile([under('SystemRoot', 'System32\\cmd.exe')]), []],
		[
			'git-bash',
			await firstFile([
				under('ProgramFiles', 'Git\\bin\\bash.exe'),
				under('ProgramFiles(x86)', 'Git\\bin\\bash.exe'),
				under('LOCALAPPDATA', 'Programs\\Git\\bin\\bash.exe'),
				gitBash,
			]),
			['--login', '-i'],
		],
	];
	return shells.flatMap(([kind, executable, shellArguments]) =>
		executable === null ? [] : [{ arguments: [...shellArguments], executable, kind, name: '' }],
	);
}

async function posixShells(): Promise<DesktopInstalledShell[]> {
	let listed = '';
	try {
		listed = await readFile('/etc/shells', 'utf8');
	} catch {
		// A machine without the file still lists its Homebrew shells.
	}
	const candidates = [
		...listed
			.split('\n')
			.map((line) => line.trim())
			.filter((line) => line.startsWith('/')),
		...HOMEBREW_FOLDERS.flatMap((folder) => HOMEBREW_SHELLS.map((shell) => `${folder}/${shell}`)),
	];
	const seen = new Set<string>();
	const found: string[] = [];
	for (const candidate of candidates) {
		if (!(await isExecutableFile(candidate))) continue;
		let resolved = candidate;
		try {
			resolved = await realpath(candidate);
		} catch {
			// An unresolvable path still names itself.
		}
		if (seen.has(resolved)) continue;
		seen.add(resolved);
		found.push(candidate);
	}
	return found.map((executable) => {
		const name = path.posix.basename(executable);
		const shared = found.filter((other) => path.posix.basename(other) === name).length > 1;
		return {
			arguments: [],
			executable,
			kind: 'named',
			name: shared ? `${name} (${path.posix.dirname(executable)})` : name,
		};
	});
}

/**
 * The shells installed here that a new launch profile can start, as Add profile lists them: on
 * Windows, PowerShell, Windows PowerShell, Command Prompt and Git Bash; elsewhere, each shell
 * `/etc/shells` names and each Homebrew shell. The TUI lists the same shells from the same places
 * through the local runtime's `installed_shells`. It only reads the file system and never fails.
 */
export async function detectInstalledShells(
	platform: NodeJS.Platform,
	environment: NodeJS.ProcessEnv,
): Promise<DesktopInstalledShell[]> {
	try {
		const shells = platform === 'win32' ? await windowsShells(environment) : await posixShells();
		return shells.slice(0, MAXIMUM_SHELLS);
	} catch {
		return [];
	}
}
