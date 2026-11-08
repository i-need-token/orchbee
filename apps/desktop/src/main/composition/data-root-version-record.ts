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

/**
 * Whether `candidate` takes precedence over `than` as Semantic Versioning orders versions; build
 * metadata is ignored, and a version either side cannot read is never newer.
 */
function isNewerProductVersion(candidate: string, than: string): boolean {
	const left = parsePrecedence(candidate);
	const right = parsePrecedence(than);
	return left !== null && right !== null && comparePrecedence(left, right) > 0;
}

interface Precedence {
	readonly core: readonly [number, number, number];
	readonly preRelease: readonly string[];
}

function parsePrecedence(text: string): Precedence | null {
	const [version = ''] = text.split('+', 1);
	const separator = version.indexOf('-');
	const core = separator === -1 ? version : version.slice(0, separator);
	const preRelease = separator === -1 ? [] : version.slice(separator + 1).split('.');
	const numbers = core.split('.');
	if (numbers.length !== 3 || numbers.some((part) => !/^\d+$/u.test(part))) return null;
	if (preRelease.some((part) => part.length === 0)) return null;
	const [major = 0, minor = 0, patch = 0] = numbers.map(Number);
	return { core: [major, minor, patch], preRelease };
}

function comparePrecedence(left: Precedence, right: Precedence): number {
	for (let index = 0; index < 3; index += 1) {
		const order = (left.core[index] ?? 0) - (right.core[index] ?? 0);
		if (order !== 0) return Math.sign(order);
	}
	if (left.preRelease.length === 0 || right.preRelease.length === 0) {
		return Math.sign(right.preRelease.length - left.preRelease.length) || 0;
	}
	const shared = Math.min(left.preRelease.length, right.preRelease.length);
	for (let index = 0; index < shared; index += 1) {
		const order = compareIdentifier(left.preRelease[index] ?? '', right.preRelease[index] ?? '');
		if (order !== 0) return order;
	}
	return Math.sign(left.preRelease.length - right.preRelease.length);
}

function compareIdentifier(left: string, right: string): number {
	const leftNumeric = /^\d+$/u.test(left);
	const rightNumeric = /^\d+$/u.test(right);
	if (leftNumeric && rightNumeric) return Math.sign(Number(left) - Number(right));
	if (leftNumeric) return -1;
	if (rightNumeric) return 1;
	if (left === right) return 0;
	return left < right ? -1 : 1;
}

function isJsonObject(value: unknown): value is Readonly<Record<string, unknown>> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
