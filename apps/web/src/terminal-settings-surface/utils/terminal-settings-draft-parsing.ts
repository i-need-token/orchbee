const BYTES_PER_MEGABYTE = 1_000_000;
const MAXIMUM_SCROLLBACK_BYTES = 1_000_000_000;

/** Terminal history is edited in megabytes and stored in bytes. */
export function formatScrollbackMegabytes(bytes: number): string {
	return String(Math.round((bytes / BYTES_PER_MEGABYTE) * 100) / 100);
}

/** A plain decimal megabyte count within the Host range, in whole bytes; null otherwise. */
export function parseScrollbackMegabytes(value: string): number | null {
	if (!/^(?:0|[1-9][0-9]*)(?:\.[0-9]+)?$/u.test(value.trim())) return null;
	const bytes = Math.round(Number(value) * BYTES_PER_MEGABYTE);
	return Number.isSafeInteger(bytes) && bytes <= MAXIMUM_SCROLLBACK_BYTES ? bytes : null;
}

export const SCROLLBACK_MEGABYTE_RANGE = {
	maximum: MAXIMUM_SCROLLBACK_BYTES / BYTES_PER_MEGABYTE,
	minimum: 0,
} as const;
