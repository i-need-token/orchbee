/** One registered language as Monaco lists it: the names and extensions that select it. */
export interface LanguageEntry {
	readonly id: string;
	readonly extensions?: readonly string[] | undefined;
	readonly filenames?: readonly string[] | undefined;
}

/**
 * The language a file opens in: an exact file name first, such as `Dockerfile`, then the longest
 * matching extension, such as `.d.ts` over `.ts`, compared without case; plain text otherwise.
 */
export function languageForPath(path: string, languages: readonly LanguageEntry[]): string {
	const name = (path.split('/').at(-1) ?? path).toLowerCase();
	let best: { readonly id: string; readonly length: number } | null = null;
	for (const language of languages) {
		if (language.filenames?.some((filename) => filename.toLowerCase() === name) === true) {
			return language.id;
		}
		for (const extension of language.extensions ?? []) {
			const lowered = extension.toLowerCase();
			if (name.endsWith(lowered) && (best === null || lowered.length > best.length)) {
				best = { id: language.id, length: lowered.length };
			}
		}
	}
	return best?.id ?? 'plaintext';
}
