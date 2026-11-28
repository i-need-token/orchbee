/** The names VS Code's desktop `files.exclude` hides by default. */
const DEFAULT_EXCLUDED_NAMES: ReadonlySet<string> = new Set([
	'.git',
	'.svn',
	'.hg',
	'.jj',
	'.DS_Store',
	'Thumbs.db',
]);

/**
 * Whether VS Code's default `files.exclude` hides an entry from the Explorer. Its `**` patterns name
 * the entry exactly, case included, on every platform, so `.GIT` and `.gitignore` show.
 */
export function isExcludedByDefault(name: string): boolean {
	return DEFAULT_EXCLUDED_NAMES.has(name);
}
