import type { FilePathRules } from './file-resource';
import { filePathKey } from './filePathKey';

const WINDOWS_DRIVE = /^[A-Za-z]:$/u;

function isRootPath(path: string, rules: FilePathRules): boolean {
	if (path === rules.separator) return true;
	return (
		rules.os === 'windows' &&
		path.endsWith(rules.separator) &&
		WINDOWS_DRIVE.test(path.slice(0, -1))
	);
}

/** The folder that holds a path, or null for a root such as `/` or `C:\`. */
export function filePathParent(path: string, rules: FilePathRules): string | null {
	const index = path.lastIndexOf(rules.separator);
	if (index < 0 || isRootPath(path, rules)) return null;
	const parent = path.slice(0, index);
	if (parent === '') return rules.separator;
	if (rules.os === 'windows' && WINDOWS_DRIVE.test(parent)) return `${parent}${rules.separator}`;
	return parent;
}

/** A path's last segment, its entry's name. */
export function filePathName(path: string, rules: FilePathRules): string {
	return path.slice(path.lastIndexOf(rules.separator) + 1);
}

/** The path of the entry `name` in `folder`. */
export function filePathChild(folder: string, name: string, rules: FilePathRules): string {
	return folder.endsWith(rules.separator)
		? `${folder}${name}`
		: `${folder}${rules.separator}${name}`;
}

/** Whether `path` is `ancestor` or lies below it, compared by the machine's rules. */
export function isFilePathWithin(path: string, ancestor: string, rules: FilePathRules): boolean {
	const key = filePathKey(path, rules);
	const ancestorKey = filePathKey(ancestor, rules);
	if (key === ancestorKey) return true;
	const prefix = ancestorKey.endsWith(rules.separator)
		? ancestorKey
		: `${ancestorKey}${rules.separator}`;
	return key.startsWith(prefix);
}
