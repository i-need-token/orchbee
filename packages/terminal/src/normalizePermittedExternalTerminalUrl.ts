/** Normalize one external terminal URL through the accepted product allowlist. */
export function normalizePermittedExternalTerminalUrl(target: string): string | null {
	let url: URL;
	try {
		url = new URL(target);
	} catch {
		return null;
	}
	if (url.protocol !== 'http:' && url.protocol !== 'https:' && url.protocol !== 'mailto:') {
		return null;
	}
	return url.href;
}
