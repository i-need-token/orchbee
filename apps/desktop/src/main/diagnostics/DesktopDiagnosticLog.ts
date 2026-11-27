/**
 * Desktop Main's records in the data root's daily diagnostic log, under the rules the native
 * writers follow. Cross-runtime contract: search for `daily-diagnostic-log` before changing the
 * record shape, a limit, the retention rule, or the file names.
 *
 * Event names and text values are tokens from a closed grammar and the other values are counts and
 * flags, so a record cannot carry terminal content, typed input, prompts, arguments, environment or
 * configuration values, file contents, or credentials. Writing is best effort: a failure is
 * dropped and never changes what Main does.
 */
import {
	closeSync,
	constants,
	fstatSync,
	lstatSync,
	mkdirSync,
	openSync,
	readSync,
	readdirSync,
	rmSync,
	writeSync,
} from 'node:fs';
import path from 'node:path';

const DAILY_LOG_LIMIT_BYTES = 7 * 1024 * 1024;
const RETAINED_LOG_DAYS = 7;
/** Room kept below the limit for the one `log-capped` record. */
const CAPPED_RESERVE_BYTES = 256;
const TAIL_BYTES = 1024;
const CAPPED_EVENT = 'log-capped';
const MAXIMUM_FIELDS = 8;
const TOKEN = /^[A-Za-z0-9._-]{1,64}$/u;
const FIELD_NAME = /^[a-z][A-Za-z0-9]{0,31}$/u;
const DAILY_FILE = /^(\d{4})-(\d{2})-(\d{2})\.jsonl$/u;
/** libuv's Windows open without sharing, so a native writer waits while Main appends. */
const UV_FS_O_EXLOCK = 0x1000_0000;
/**
 * macOS `O_EXLOCK`: the open takes the file's exclusive `flock` lock, the one the natives hold while
 * they append, and with `O_NONBLOCK` answers `EAGAIN` while another writer holds it.
 */
const DARWIN_O_EXLOCK = 0x20;
/** Access for anyone but the owner, which a private log file and folder never grant. */
const OTHERS_ACCESS = 0o077;
const SHARING_WAIT_MILLISECONDS = 500;
const SHARING_RETRY_MILLISECONDS = 10;
const DAY_MILLISECONDS = 86_400_000;
/** Whether the open appends (platformOpenFlags) rather than writing at the size it read. */
const appends = process.platform !== 'win32' && process.platform !== 'darwin';

type DesktopLogLevel = 'error' | 'info' | 'warning';
type DesktopLogValue = boolean | number | string;
/**
 * Text written in the source: a string literal type, or a union of them such as Electron's renderer
 * exit reasons, never a runtime `string`, as the native writers take only `&'static str`. Text
 * outside the token grammar (letters, digits, `.`, `_`, `-`, at most 64 characters) is still
 * written as `invalid`.
 */
type SourceText<Value> = Value extends string ? (string extends Value ? never : Value) : Value;
type SourceFields<Fields> = { readonly [Name in keyof Fields]: SourceText<Fields[Name]> };

/** Appends one record whose event name and text values are written in the source. */
export type DesktopDiagnosticRecorder = <
	const Event extends string,
	const Fields extends Readonly<Record<string, DesktopLogValue>> = Readonly<Record<never, never>>,
>(
	level: DesktopLogLevel,
	event: Event & SourceText<Event>,
	fields?: Fields & SourceFields<Fields>,
) => void;

interface DesktopDiagnosticLogClock {
	/** Milliseconds since the Unix epoch. */
	now(): number;
	/** Days since 1970-01-01 of the local date at `milliseconds`. */
	localDay(milliseconds: number): number;
}

const systemClock: DesktopDiagnosticLogClock = {
	localDay: (milliseconds) => {
		const date = new Date(milliseconds);
		return Math.floor(
			Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / DAY_MILLISECONDS,
		);
	},
	now: () => Date.now(),
};

export class DesktopDiagnosticLog {
	readonly #clock: DesktopDiagnosticLogClock;
	readonly #directory: string;
	#prunedDay: number | undefined;

	constructor(directory: string, clock: DesktopDiagnosticLogClock = systemClock) {
		this.#directory = directory;
		this.#clock = clock;
	}

	/** The recorder Main's events go through. */
	recorder(): DesktopDiagnosticRecorder {
		return (level, event, fields) => this.#append(level, event, fields ?? {});
	}

	#append(
		level: DesktopLogLevel,
		event: string,
		fields: Readonly<Record<string, DesktopLogValue>>,
	): void {
		try {
			const now = this.#clock.now();
			const day = this.#clock.localDay(now);
			this.#write(day, encode(now, level, event, fields), encode(now, 'warning', CAPPED_EVENT, {}));
		} catch {
			// The log never changes what Main does.
		}
	}

	#write(day: number, record: string, capped: string): void {
		mkdirSync(this.#directory, { mode: 0o700, recursive: true });
		if (process.platform !== 'win32') requirePrivateFolder(this.#directory);
		const descriptor = openExclusive(path.join(this.#directory, dailyFileName(day)));
		try {
			if (process.platform !== 'win32') requirePrivateFile(descriptor);
			if (this.#prunedDay !== day) {
				this.#prunedDay = day;
				pruneDailyFiles(this.#directory, day);
			}
			const size = fstatSync(descriptor).size;
			const line = admittedLine(size, readTail(descriptor, size), record, capped);
			// An `O_APPEND` descriptor writes at the end of the file wherever it stands.
			if (line !== undefined) writeSync(descriptor, line, appends ? null : size, 'utf8');
		} finally {
			closeSync(descriptor);
		}
	}
}

/** The record to write: the record itself, the one capped record, or nothing once capped. */
function admittedLine(
	size: number,
	tail: string,
	record: string,
	capped: string,
): string | undefined {
	if (endsCapped(tail)) return undefined;
	return size + Buffer.byteLength(record) + CAPPED_RESERVE_BYTES <= DAILY_LOG_LIMIT_BYTES
		? record
		: capped;
}

/** Removes daily files dated more than six days before `today`; any other name stays. */
function pruneDailyFiles(directory: string, today: number): void {
	for (const entry of readdirSync(directory, { withFileTypes: true })) {
		const day = dailyFileDay(entry.name);
		if (day === undefined || day >= today - (RETAINED_LOG_DAYS - 1) || !entry.isFile()) continue;
		try {
			rmSync(path.join(directory, entry.name));
		} catch {
			// Another writer may hold or have removed it.
		}
	}
}

function dailyFileName(day: number): string {
	return `${new Date(day * DAY_MILLISECONDS).toISOString().slice(0, 10)}.jsonl`;
}

function dailyFileDay(name: string): number | undefined {
	const match = DAILY_FILE.exec(name);
	if (match === null) return undefined;
	const [, year, month, date] = match.map(Number);
	if (year === undefined || month === undefined || date === undefined) return undefined;
	const milliseconds = Date.UTC(year, month - 1, date);
	if (dailyFileName(milliseconds / DAY_MILLISECONDS) !== name) return undefined;
	return milliseconds / DAY_MILLISECONDS;
}

/** One record line. Tokens need no escaping: the grammar admits no quote, backslash, or control. */
function encode(
	milliseconds: number,
	level: DesktopLogLevel,
	event: string,
	fields: Readonly<Record<string, DesktopLogValue>>,
): string {
	const body = Object.entries(fields)
		.filter(([name]) => FIELD_NAME.test(name))
		.slice(0, MAXIMUM_FIELDS)
		.map(([name, value]) => `"${name}":${encodeValue(value)}`)
		.join(',');
	const token = TOKEN.test(event) ? event : 'invalid';
	return `{"time":"${new Date(milliseconds).toISOString()}","level":"${level}","role":"desktop-main","pid":${process.pid},"event":"${token}","fields":{${body}}}\n`;
}

function encodeValue(value: DesktopLogValue): string {
	if (typeof value === 'boolean') return String(value);
	if (typeof value === 'number') {
		return Number.isSafeInteger(value) && value >= 0 ? String(value) : '"invalid"';
	}
	return TOKEN.test(value) ? `"${value}"` : '"invalid"';
}

function endsCapped(tail: string): boolean {
	if (!tail.endsWith('\n')) return false;
	const body = tail.slice(0, -1);
	const line = body.slice(body.lastIndexOf('\n') + 1);
	try {
		const record: unknown = JSON.parse(line);
		return (
			typeof record === 'object' &&
			record !== null &&
			'event' in record &&
			record.event === CAPPED_EVENT
		);
	} catch {
		return false;
	}
}

function readTail(descriptor: number, size: number): string {
	const length = Math.min(size, TAIL_BYTES);
	if (length === 0) return '';
	const buffer = Buffer.alloc(length);
	const read = readSync(descriptor, buffer, 0, length, size - length);
	return buffer.subarray(0, read).toString('utf8');
}

/**
 * Opens the day's file held against every other writer, waiting briefly while another writer holds
 * it: without sharing on Windows, and on macOS under its exclusive lock, never through a link. Linux
 * opens it to append without the lock.
 */
function openExclusive(file: string): number {
	const flags = constants.O_RDWR | constants.O_CREAT | platformOpenFlags();
	const deadline = Date.now() + SHARING_WAIT_MILLISECONDS;
	for (;;) {
		try {
			return openSync(file, flags, 0o600);
		} catch (error) {
			const code = error instanceof Error && 'code' in error ? error.code : undefined;
			const busy = code === 'EBUSY' || code === 'EAGAIN';
			if (!busy || Date.now() >= deadline) throw error;
			Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, SHARING_RETRY_MILLISECONDS);
		}
	}
}

/**
 * The open's hold on the file. Node cannot take the natives' `flock` lock on Linux, so Linux Main
 * appends through `O_APPEND` without it, as the natives append under it: no record overwrites
 * another, and only the records appended while Main reads the file's size can carry the day past its
 * limit.
 */
function platformOpenFlags(): number {
	switch (process.platform) {
		case 'win32':
			return UV_FS_O_EXLOCK;
		case 'darwin':
			return DARWIN_O_EXLOCK | constants.O_NONBLOCK | constants.O_NOFOLLOW;
		default:
			return constants.O_APPEND | constants.O_NOFOLLOW;
	}
}

/** The natives' folder rule: a real folder, owned by this user, that grants nobody else access. */
function requirePrivateFolder(directory: string): void {
	const status = lstatSync(directory);
	if (!status.isDirectory() || !ownedPrivately(status.uid, status.mode)) {
		throw new Error('The log folder is not private.');
	}
}

/** The natives' file rule: a single-link regular file, owned by this user, private to it. */
function requirePrivateFile(descriptor: number): void {
	const status = fstatSync(descriptor);
	if (!status.isFile() || status.nlink !== 1 || !ownedPrivately(status.uid, status.mode)) {
		throw new Error('The daily log file is not private.');
	}
}

function ownedPrivately(uid: number, mode: number): boolean {
	return uid === process.getuid?.() && (mode & OTHERS_ACCESS) === 0;
}
