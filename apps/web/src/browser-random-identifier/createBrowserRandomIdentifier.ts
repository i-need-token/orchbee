const RANDOM_IDENTIFIER_BYTES = 16;

type BrowserRandomValueFiller = (values: Uint8Array<ArrayBuffer>) => void;

export function createBrowserRandomIdentifier(
	fillRandomValues: BrowserRandomValueFiller = (values) => {
		globalThis.crypto.getRandomValues(values);
	},
): string {
	const values = new Uint8Array(RANDOM_IDENTIFIER_BYTES);
	fillRandomValues(values);
	return [...values].map((value) => value.toString(16).padStart(2, '0')).join('');
}
