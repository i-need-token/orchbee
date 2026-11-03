/** The Worktree path rules the native Clients follow, kept identical so every Client agrees. */

const MAXIMUM_DIRECTORY_PATH_BYTES = 4096;
const utf8 = new TextEncoder();

function lowerAscii(value: string): string {
	return value.replace(/[A-Z]/gu, (letter) => letter.toLowerCase());
}

function windowsSpelling(path: string): string {
	return lowerAscii(path.replaceAll('\\', '/').replace(/\/+$/u, ''));
}

/**
 * Whether two directory spellings name the same Worktree directory under the association rule.
 * POSIX spellings compare exactly, because a volume may be case sensitive. Windows spellings may
 * differ only in separators, trailing separators and ASCII case. A POSIX spelling never equals a
 * Windows one. Mirrors `same_directory_spelling` in the Rust Workbench protocol.
 */
export function sameDirectorySpelling(left: string, right: string): boolean {
	if (left.startsWith('/') || right.startsWith('/')) return left === right;
	return windowsSpelling(left) === windowsSpelling(right);
}

function slug(branch: string): string {
	let result = '';
	for (const character of branch) {
		if (/^[A-Za-z0-9]$/u.test(character)) result += character.toLowerCase();
		else if (result !== '' && !result.endsWith('-')) result += '-';
	}
	result = result.replace(/-+$/u, '');
	return result === '' ? 'worktree' : result;
}

/**
 * Where a configured Worktrees directory places a new checkout: the root, the repository's name and
 * a slug of the branch. Null when the result exceeds the protocol's path bound. Mirrors
 * `worktree_destination::destination` in the native Clients, on the platform the paths belong to.
 */
export function worktreeDestination(
	root: string,
	repositoryRoot: string,
	branch: string,
): string | null {
	const windows = !root.startsWith('/');
	const separators = windows ? /[\\/]/u : /\//u;
	const separator = windows ? '\\' : '/';
	const name = repositoryRoot
		.split(separators)
		.filter((component) => component !== '' && component !== '.')
		.at(-1);
	const repository =
		name === undefined || name === '..' || (windows && /^[A-Za-z]:$/u.test(name))
			? 'repository'
			: name;
	const joined = separators.test(root.at(-1) ?? '') ? root : `${root}${separator}`;
	const path = `${joined}${repository}${separator}${slug(branch)}`;
	return utf8.encode(path).length <= MAXIMUM_DIRECTORY_PATH_BYTES && !path.includes('\0')
		? path
		: null;
}
