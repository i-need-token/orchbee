/**
 * Whether every term of a Settings query appears in the given texts. Matching ignores case and
 * compatibility forms in the effective locale; an empty query matches everything.
 */
export function matchesSettingsQuery(
	query: string,
	texts: readonly string[],
	locale: string,
): boolean {
	const terms = normalize(query, locale).split(/\s+/u).filter(Boolean);
	if (terms.length === 0) return true;
	const document = normalize(texts.join(' '), locale);
	return terms.every((term) => document.includes(term));
}

function normalize(text: string, locale: string): string {
	return text.normalize('NFKC').toLocaleLowerCase(locale);
}
