import type { FilePathRules } from '../../file-session';

/**
 * A path as VS Code's Copy Relative Path writes it in a folder opened on its own: below the root, in
 * the machine's spelling, and empty for the root itself.
 */
export function explorerRelativePath(path: string, root: string, rules: FilePathRules): string {
	if (path === root) return '';
	const prefix = root.endsWith(rules.separator) ? root : `${root}${rules.separator}`;
	return path.startsWith(prefix) ? path.slice(prefix.length) : path;
}
