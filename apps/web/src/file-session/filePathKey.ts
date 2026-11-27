import type { FilePathRules } from './file-resource';

/**
 * A path's comparison key on a machine with these rules: its spelling as given, lowercased where
 * the machine ignores case, as VS Code's comparison keys do. Nothing else is normalized.
 */
export function filePathKey(path: string, rules: FilePathRules): string {
	return rules.caseSensitive ? path : path.toLowerCase();
}
