import type { languages } from './monaco-entry';

/** Monaco's language for text that no association claims. */
const PLAIN_TEXT = 'plaintext';

/**
 * The language a file opens in, by VS Code's association rule without detection from content: an
 * exact file name wins, then the longest extension the name ends with, both ignoring case, and the
 * language declared last wins a tie. A file nothing claims opens as plain text.
 */
export function languageIdForPath(
	declared: readonly languages.ILanguageExtensionPoint[],
	path: string,
): string {
	const name = path
		.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1)
		.toLowerCase();
	let byExtension: { readonly id: string; readonly length: number } | undefined;
	for (let index = declared.length - 1; index >= 0; index -= 1) {
		const language = declared[index];
		if (language === undefined) continue;
		if (language.filenames?.some((filename) => filename.toLowerCase() === name) === true) {
			return language.id;
		}
		for (const extension of language.extensions ?? []) {
			const suffix = extension.toLowerCase();
			if (!name.endsWith(suffix)) continue;
			if (byExtension === undefined || suffix.length > byExtension.length) {
				byExtension = { id: language.id, length: suffix.length };
			}
		}
	}
	return byExtension?.id ?? PLAIN_TEXT;
}
