/** A new change's ID, 128 random bits in hex, so the backend runs it once however often it hears it. */
export function gitRequestId(): string {
	const bytes = crypto.getRandomValues(new Uint8Array(16));
	return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}
