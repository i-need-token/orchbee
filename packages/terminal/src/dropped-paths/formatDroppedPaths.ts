/** The family of the machine whose shell receives the paths: Windows, or any POSIX system. */
export type DroppedPathPlatform = 'posix' | 'windows';

/** Characters a POSIX shell reads specially, which Terminal.app escapes with a backslash. */
const POSIX_SPECIAL = /[\s"'\\$`!&;|<>()[\]{}*?#~^]/gu;

function holdsControlCharacter(path: string): boolean {
	for (const character of path) {
		const code = character.codePointAt(0) ?? 0;
		if (code < 0x20 || code === 0x7f) return true;
	}
	return false;
}

function posixPath(path: string): string {
	// A backslash before a line break would continue the line, so such a path is single-quoted.
	if (holdsControlCharacter(path)) return `'${path.replaceAll("'", String.raw`'\''`)}'`;
	return path.replaceAll(POSIX_SPECIAL, String.raw`\$&`);
}

/**
 * The paths as the platform's own terminal pastes a drop: Windows Terminal quotes a path that holds
 * a space and joins them with spaces; Terminal.app escapes each special character and ends every
 * path with a space.
 */
export function formatDroppedPaths(
	paths: readonly string[],
	platform: DroppedPathPlatform,
): string {
	if (platform === 'windows') {
		return paths.map((path) => (path.includes(' ') ? `"${path}"` : path)).join(' ');
	}
	return paths.map((path) => `${posixPath(path)} `).join('');
}
