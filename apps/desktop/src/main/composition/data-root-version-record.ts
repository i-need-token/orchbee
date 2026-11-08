import { isNewerProductVersion } from '../product-version';

// Cross-runtime contract: search for `data-root-version-record` before changing this shape; native
// writers read and write the same record.
/** The layout this build reads and writes; a root recording a higher one is refused. */
export const DATA_ROOT_LAYOUT = 1;
const RECORD_LIMIT_BYTES = 4096;

/**
 * A data root this version will not open: its record names a higher layout, or cannot be read.
 * Nothing in the root is read or written after it.
 */
export class DesktopDataRootRefusal extends Error {
	/** The higher layout, or `undefined` when the record cannot be read. */
	readonly layout: number | undefined;
	readonly newestVersion: string | undefined;
	readonly recordPath: string;
	readonly root: string;

	constructor(
		root: string,
		recordPath: string,
		record: { readonly layout: number; readonly newestVersion: string | undefined } | undefined,
	) {
		super('The data root layout is refused.');
		this.name = 'DesktopDataRootRefusal';
		this.layout = record?.layout;
		this.newestVersion = record?.newestVersion;
		this.recordPath = recordPath;
		this.root = root;
	}
}

export interface DataRootVersionRecord {
	/** Every entry, so raising `newestVersion` keeps those a later release added. */
	readonly entries: Readonly<Record<string, unknown>>;
	readonly layout: number;
	readonly newestVersion: string | undefined;
}

/**
 * A JSON object whose `layout` is a positive integer and whose `newestVersion`, when present, is a
 * string; any other entry is kept unread.
 */
export function parseDataRootVersionRecord(text: string): DataRootVersionRecord | null {
	if (Buffer.byteLength(text, 'utf8') > RECORD_LIMIT_BYTES) return null;
	let value: unknown;
	try {
		value = JSON.parse(text);
	} catch {
		return null;
	}
	if (!isJsonObject(value)) return null;
	const entries = value;
	const layout = entries['layout'];
	if (typeof layout !== 'number' || !Number.isSafeInteger(layout) || layout < 1) return null;
	const newestVersion = entries['newestVersion'];
	if (newestVersion !== undefined && typeof newestVersion !== 'string') return null;
	return { entries, layout, newestVersion };
}

/** The record with `newestVersion` raised to `version`, or `null` when it is already as new. */
export function raiseDataRootVersionRecord(
	record: DataRootVersionRecord,
	version: string,
): string | null {
	if (record.newestVersion !== undefined && !isNewerProductVersion(version, record.newestVersion)) {
		return null;
	}
	return `${JSON.stringify({ ...record.entries, newestVersion: version })}\n`;
}

function isJsonObject(value: unknown): value is Readonly<Record<string, unknown>> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
