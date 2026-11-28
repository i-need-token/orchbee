/** What a tab reads: its file's name, and the folder part that tells it from tabs of the same name. */
export interface FileTabLabel {
	/** Empty unless another tab shows the same name from another folder. */
	readonly description: string;
	readonly name: string;
}

const ELLIPSIS = '\u2026';
const UNC_PREFIX = '\\\\';

/** Windows paths, drive or UNC, separate with a backslash; every other path with a slash. */
function separatorOf(path: string): '/' | '\\' {
	return /^[A-Za-z]:[\\/]/u.test(path) || path.startsWith(UNC_PREFIX) ? '\\' : '/';
}

function splitPath(path: string): { readonly folder: string; readonly name: string } {
	const separator = separatorOf(path);
	const end = path.lastIndexOf(separator);
	if (end < 0) return { folder: '', name: path };
	const name = path.slice(end + 1);
	const folder = path.slice(0, end);
	// A file at a root keeps the root as its folder: `/` or `C:\`.
	if (folder === '' || /^[A-Za-z]:$/u.test(folder)) return { folder: folder + separator, name };
	return { folder, name };
}

/**
 * Labels the row's tabs as VS Code's tabs do by default: each shows its file's name, and tabs that
 * share a name show the shortest part of their folders that no other of those folders contains.
 * Until the overlay knows its Workspace's root, a folder is the file's absolute folder, which VS
 * Code shows only when the relative ones collide.
 */
export function fileTabLabels(paths: readonly string[]): readonly FileTabLabel[] {
	const parts = paths.map(splitPath);
	const labels = parts.map((part) => ({ description: '', name: part.name }));
	const byName = new Map<string, number[]>();
	parts.forEach((part, index) => {
		const indexes = byName.get(part.name);
		if (indexes === undefined) byName.set(part.name, [index]);
		else indexes.push(index);
	});
	for (const indexes of byName.values()) {
		if (indexes.length < 2) continue;
		const folders = [...new Set(indexes.map((index) => parts[index]?.folder ?? ''))];
		// Tabs of one name in one folder need no description to tell them apart.
		if (folders.length < 2) continue;
		const descriptions = shortenFolders(folders);
		for (const index of indexes) {
			const folder = parts[index]?.folder ?? '';
			labels[index] = {
				description: descriptions[folders.indexOf(folder)] ?? folder,
				name: parts[index]?.name ?? '',
			};
		}
	}
	return labels;
}

/**
 * The shortest run of folder names that no other folder contains, searched from the end, with an
 * ellipsis for each side it leaves out and the drive or root kept in front; a folder with no such
 * run shows whole.
 */
function shortenFolders(folders: readonly string[]): readonly string[] {
	return folders.map((folder, index) => {
		const separator = separatorOf(folder);
		if (folder === '') return `.${separator}`;
		let prefix = '';
		let rest = folder;
		if (rest.startsWith(UNC_PREFIX)) {
			prefix = UNC_PREFIX;
			rest = rest.slice(UNC_PREFIX.length);
		} else if (rest.startsWith(separator)) {
			prefix = separator;
			rest = rest.slice(separator.length);
		} else if (rest.startsWith('~')) {
			prefix = '~';
			rest = rest.slice(1);
		}
		const segments = rest.split(separator);
		const others = folders.filter((_, otherIndex) => otherIndex !== index);
		for (let length = 1; length <= segments.length; length += 1) {
			for (let start = segments.length - length; start >= 0; start -= 1) {
				const run = segments.slice(start, start + length).join(separator);
				const endsFolder = start + length === segments.length;
				const contained = others.some((other) => {
					if (other === '' || !other.includes(run)) return false;
					// A run that ends the folder only collides with a folder that ends the same way.
					if (!endsFolder) return true;
					const bounded = start > 0 && other.includes(separator) ? separator + run : run;
					return other.endsWith(bounded);
				});
				if (contained) continue;
				return shortened(segments, start, length, prefix, separator);
			}
		}
		return folder;
	});
}

function shortened(
	segments: readonly string[],
	runStart: number,
	runLength: number,
	prefix: string,
	separator: string,
): string {
	let start = runStart;
	let length = runLength;
	let lead = '';
	const first = segments[0] ?? '';
	if (first.endsWith(':') || prefix !== '') {
		// The drive or root stays in front; a run right after it absorbs it.
		if (start === 1) {
			start = 0;
			length += 1;
		}
		lead = prefix + (start > 0 ? first + separator : '');
	}
	if (start > 0) lead += ELLIPSIS + separator;
	let label = lead + segments.slice(start, start + length).join(separator);
	if (start + length < segments.length) label += separator + ELLIPSIS;
	return label;
}
