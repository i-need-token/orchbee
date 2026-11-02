import { workbenchTabMessages, type IntlShape } from '@orchbee/i18n';

/**
 * What Windows programs such as cmd put before an elevated console's title, in the display
 * language's own wording; only `en-US` and `zh-CN` are known, `zh-CN` with either colon.
 */
const ELEVATION_PREFIXES = ['Administrator: ', '管理员: ', '管理员：'] as const;

/** One of these, alone or before a space, is a program's sign of activity rather than its title. */
const ACTIVITY_MARK = /^(?:[⠀-⣿]|[·✢✳✶✻✽◐◓◑◒])(?:\s|$)/u;

const ABSOLUTE_DRIVE_PATH = /^[A-Za-z]:[\\/]/u;

const EXECUTABLE_EXTENSIONS: ReadonlySet<string> = new Set(['exe', 'com', 'bat', 'cmd']);

/**
 * Names a terminal by the title its program set, as the TUI's chrome does. A console host titles a
 * window with the absolute path of its launched program until that program sets a title, and cmd
 * extends it to `<path> - <command>` while it runs one. That path is launch identity rather than a
 * task, so the program's name replaces it and the command stays. Null means nothing is left.
 */
export function nameServerTerminalTitle(title: string, intl: IntlShape): string | null {
	const stripped = stripTerminalTitle(title);
	if (stripped === null) return null;
	const program = nameConsoleProgram(stripped, intl);
	if (program !== null) return program;
	for (const separator of stripped.matchAll(/ - /gu)) {
		const name = nameConsoleProgram(stripped.slice(0, separator.index), intl);
		if (name === null) continue;
		const command = stripped.slice(separator.index + separator[0].length).trim();
		return command.length === 0 ? name : `${name} - ${command}`;
	}
	return stripped;
}

/**
 * Names the program at a path written with either separator by one rule on every platform: a known
 * shell by its product, anything else by its file name without a Windows executable extension.
 */
export function nameServerTerminalProgram(path: string, intl: IntlShape): string | null {
	const name = fileName(path);
	const file = executableStem(name) ?? name;
	return file.trim().length === 0 ? null : (shellName(file, intl) ?? file);
}

/** Drops one elevation prefix, then one activity mark. */
function stripTerminalTitle(title: string): string | null {
	const prefix = ELEVATION_PREFIXES.find((candidate) => title.startsWith(candidate));
	const unprefixed = (prefix === undefined ? title : title.slice(prefix.length)).trim();
	const [mark] = unprefixed;
	const stripped =
		mark !== undefined && ACTIVITY_MARK.test(unprefixed)
			? unprefixed.slice(mark.length).trim()
			: unprefixed;
	return stripped.length === 0 ? null : stripped;
}

/** A console default title is an absolute drive or UNC path to a Windows executable. */
function nameConsoleProgram(title: string, intl: IntlShape): string | null {
	const absolute = ABSOLUTE_DRIVE_PATH.test(title) || title.startsWith('\\\\');
	return absolute && executableStem(fileName(title)) !== null
		? nameServerTerminalProgram(title, intl)
		: null;
}

/**
 * Shells are named by product, which stays untranslated except for the name Windows gives cmd.
 * Matching ignores ASCII case, as Windows file names do, so every platform applies one rule.
 */
function shellName(file: string, intl: IntlShape): string | null {
	switch (asciiLowercase(file)) {
		case 'pwsh':
			return 'PowerShell';
		case 'powershell':
			return 'Windows PowerShell';
		case 'cmd':
			return intl.formatMessage(workbenchTabMessages.commandPromptTitle);
		case 'bash':
			return 'Bash';
		case 'zsh':
			return 'Zsh';
		default:
			return null;
	}
}

function fileName(path: string): string {
	return path.slice(Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/')) + 1);
}

function executableStem(name: string): string | null {
	const dot = name.lastIndexOf('.');
	return dot > 0 && EXECUTABLE_EXTENSIONS.has(asciiLowercase(name.slice(dot + 1)))
		? name.slice(0, dot)
		: null;
}

function asciiLowercase(value: string): string {
	return value.replaceAll(/[A-Z]/gu, (letter) => letter.toLowerCase());
}
