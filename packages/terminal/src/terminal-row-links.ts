import type { TerminalBufferRow } from './buffer-rows';
import { normalizePermittedExternalTerminalUrl } from './normalizePermittedExternalTerminalUrl';
import type { TerminalOpenTarget } from './terminal-contracts';
import type { ILink, ILinkRange } from './types';

const PATH_COMPONENT = String.raw`[\p{L}\p{N}\p{M}_.@%+,-]+`;
const PATH_SEPARATOR = String.raw`[\\/]`;
const FILE_EXTENSION = String.raw`(?:[\p{L}\p{N}]{2,7}|[ahcmo])`;
const BARE_FILE = String.raw`[\p{L}\p{N}\p{M}_@%+,-][\p{L}\p{N}\p{M}_.@%+,-]*\.${FILE_EXTENSION}`;
const PATH_REGEX = new RegExp(
	String.raw`(?:file:\/\/(?:[A-Za-z]:|${PATH_COMPONENT})?(?:${PATH_SEPARATOR}${PATH_COMPONENT})+|[A-Za-z]:${PATH_SEPARATOR}(?:${PATH_COMPONENT}${PATH_SEPARATOR})*${PATH_COMPONENT}|(?:~|\.{1,2})?${PATH_SEPARATOR}(?:${PATH_COMPONENT}${PATH_SEPARATOR})*${PATH_COMPONENT}|(?:${PATH_COMPONENT}${PATH_SEPARATOR})+${PATH_COMPONENT}|${BARE_FILE})(?::(\d+)(?::(\d+))?)?`,
	'gu',
);
const DIRECTORY_WORD = String.raw`[\p{L}\p{N}\p{M}_@%+,-]+`;
const PATH_COMPONENT_WITH_SPACES = String.raw`(?:${PATH_COMPONENT}|${DIRECTORY_WORD}(?:(?: |\\ )${DIRECTORY_WORD})+)`;
const SPACED_BARE_FILE = String.raw`${PATH_COMPONENT}(?:(?: |\\ )${PATH_COMPONENT})*\.${FILE_EXTENSION}`;
const SPACED_FILE_PATH_REGEX = new RegExp(
	String.raw`(?:file:\/\/(?:[A-Za-z]:)?${PATH_SEPARATOR}|[A-Za-z]:${PATH_SEPARATOR}|(?:~|\.{1,2})${PATH_SEPARATOR}|(?<![\p{L}\p{N}\p{M}_~/:])\/(?!\/)|(?:${PATH_COMPONENT}${PATH_SEPARATOR})+)(?:${PATH_COMPONENT_WITH_SPACES}${PATH_SEPARATOR})*${SPACED_BARE_FILE}(?::(\d+)(?::(\d+))?)?(?=$|[\s"'),;!?\]])`,
	'gu',
);
const SHELL_ESCAPED_PATH_COMPONENT = String.raw`${PATH_COMPONENT}(?:\\ ${PATH_COMPONENT})+`;
const SHELL_PATH_COMPONENT = String.raw`(?:${PATH_COMPONENT}|${SHELL_ESCAPED_PATH_COMPONENT})`;
const SHELL_ESCAPED_PATH_REGEX = new RegExp(
	String.raw`(?:(?:~|\.{1,2})\/|(?<![\p{L}\p{N}\p{M}_~/:])\/(?!\/)|(?:${PATH_COMPONENT}\/)+)(?:${SHELL_PATH_COMPONENT}\/)*${SHELL_ESCAPED_PATH_COMPONENT}(?:\/${SHELL_PATH_COMPONENT})*(?::(\d+)(?::(\d+))?)?(?=$|[\s"'),;!?\]])`,
	'gu',
);
const QUOTED_PATH_REGEX = /(["'])([^"'\r\n]+)\1(?::(\d+)(?::(\d+))?)?/gu;
const TRAILING_PATH_PUNCTUATION = /[.,;!?)\]，。！？）】]+$/u;
const TRAILING_URL_PUNCTUATION = /[.,;!?)\]]+$/u;
const URL_REGEX =
	/(?:https?:\/\/|mailto:|ftp:\/\/|ssh:\/\/|git:\/\/|tel:|magnet:|gemini:\/\/|gopher:\/\/|news:)[\w\-.~:/?#@!$&*+,;=%]+/giu;

export interface LogicalTerminalText {
	readonly positions: readonly { readonly x: number; readonly y: number }[];
	readonly rows: readonly TerminalBufferRow[];
	readonly text: string;
}

/** Detect explicit, URL, and path links from one bounded contiguous logical-row window. */
export function detectTerminalRowLinks(
	rows: readonly TerminalBufferRow[],
	currentDirectory = '',
): readonly ILink[] {
	if (rows.length === 0) return [];
	const explicit = detectExplicitLinks(rows, currentDirectory);
	const logicalLines = buildLogicalTerminalTexts(rows);
	const urls = logicalLines
		.flatMap(detectUrlLinks)
		.filter(isPermittedExternalTerminalTarget)
		.filter((link) => !overlapsAny(link.range, explicit));
	const paths = logicalLines
		.flatMap((logical) => detectPathLinks(logical, currentDirectory))
		.filter((link) => !overlapsAny(link.range, explicit) && !overlapsAny(link.range, urls));
	return [...explicit, ...urls, ...paths];
}

export function terminalLinkContains(link: ILink, column: number, row: number): boolean {
	const { start, end } = link.range;
	if (row < start.y || row > end.y) return false;
	if (start.y === end.y) return column >= start.x && column <= end.x;
	if (row === start.y) return column >= start.x;
	if (row === end.y) return column <= end.x;
	return true;
}

export function isPermittedExternalTerminalTarget(link: ILink): boolean {
	const target = link.target;
	return target?.kind === 'url' && normalizePermittedExternalTerminalUrl(target.url) !== null;
}

export function buildLogicalTerminalTexts(
	rows: readonly TerminalBufferRow[],
): readonly LogicalTerminalText[] {
	const logicalLines: LogicalTerminalText[] = [];
	let text = '';
	const positions: Array<{ readonly x: number; readonly y: number }> = [];
	let logicalRows: TerminalBufferRow[] = [];
	for (const row of rows) {
		logicalRows.push(row);
		text += row.text;
		if (row.identityAscii) {
			for (let column = 0; column < row.text.length; column += 1) {
				positions.push({ x: column, y: Number(row.index) });
			}
		} else {
			for (const [index, mapping] of row.mappings.entries()) {
				const next = row.mappings[index + 1];
				const value = row.text.slice(mapping.utf16Offset, next?.utf16Offset ?? row.text.length);
				for (let offset = 0; offset < value.length; offset += 1) {
					positions.push({ x: mapping.column, y: Number(row.index) });
				}
			}
		}
		if (!row.wrap) {
			logicalLines.push({ positions: [...positions], rows: logicalRows, text });
			text = '';
			positions.length = 0;
			logicalRows = [];
		}
	}
	if (logicalRows.length > 0) logicalLines.push({ positions, rows: logicalRows, text });
	return logicalLines;
}

function detectExplicitLinks(
	rows: readonly TerminalBufferRow[],
	currentDirectory: string,
): readonly ILink[] {
	const links: ILink[] = [];
	let active:
		| {
				readonly target: string;
				readonly start: { readonly x: number; readonly y: number };
				end: { readonly x: number; readonly y: number };
		  }
		| undefined;
	const commit = (): void => {
		if (active === undefined) return;
		links.push({
			activation: 'modifier',
			range: { end: active.end, start: active.start },
			target: explicitTerminalTarget(active.target, currentDirectory),
			text: active.target,
		});
		active = undefined;
	};
	for (const row of rows) {
		for (const cell of row.cells ?? []) {
			if (cell.width === 0) continue;
			const point = { x: cell.column, y: Number(row.index) };
			if (cell.hyperlink === null) {
				commit();
				continue;
			}
			if (active?.target === cell.hyperlink && positionsTouch(active.end, point, rows)) {
				active.end = point;
				continue;
			}
			commit();
			active = { end: point, start: point, target: cell.hyperlink };
		}
	}
	commit();
	return links;
}

function detectUrlLinks(logical: LogicalTerminalText): readonly ILink[] {
	const links: ILink[] = [];
	URL_REGEX.lastIndex = 0;
	let match: RegExpExecArray | null = URL_REGEX.exec(logical.text);
	while (match !== null) {
		const url = match[0].replace(TRAILING_URL_PUNCTUATION, '');
		const start = logical.positions[match.index];
		const end = logical.positions[match.index + url.length - 1];
		if (url.length > 0 && start !== undefined && end !== undefined) {
			links.push({
				activation: 'modifier',
				range: { end, start },
				target: { kind: 'url', url },
				text: url,
			});
		}
		match = URL_REGEX.exec(logical.text);
	}
	return links;
}

function detectPathLinks(logical: LogicalTerminalText, currentDirectory: string): readonly ILink[] {
	const links: ILink[] = [];
	for (const match of detectPathTextMatches(logical.text)) {
		const start = logical.positions[match.index];
		const end = logical.positions[match.index + match.original.length - 1];
		const pathText = decodeDisplayedPath(match.pathText, match.quoted);
		if (pathText && start !== undefined && end !== undefined) {
			const row = logical.rows.find((candidate) => Number(candidate.index) === start.y);
			const semantic = row?.cells?.find((cell) => cell.column === start.x)?.semanticContent;
			if (
				semantic !== 'prompt' ||
				pathText.startsWith('/') ||
				pathText.startsWith('~') ||
				isWindowsAbsolutePath(pathText)
			) {
				links.push({
					activation: 'modifier',
					range: { end, start },
					target: {
						kind: 'file',
						path: resolvePath(pathText, currentDirectory),
						...(match.line === undefined ? {} : { line: match.line }),
						...(match.column === undefined ? {} : { column: match.column }),
					},
					text: match.original,
				});
			}
		}
	}
	return links;
}

interface PathTextMatch {
	readonly column: number | undefined;
	readonly index: number;
	readonly line: number | undefined;
	readonly original: string;
	readonly pathText: string;
	readonly quoted: 'double' | 'single' | 'unquoted';
}

function detectPathTextMatches(text: string): readonly PathTextMatch[] {
	const matches: PathTextMatch[] = [];
	QUOTED_PATH_REGEX.lastIndex = 0;
	let quoted: RegExpExecArray | null = QUOTED_PATH_REGEX.exec(text);
	while (quoted !== null) {
		const quote = quoted[1];
		let pathText = quoted[2] ?? '';
		let line = parseOptionalPositiveInteger(quoted[3]);
		let column = parseOptionalPositiveInteger(quoted[4]);
		if (line === undefined) {
			const location = terminalPathLocation(pathText);
			if (location !== null) {
				pathText = location.path;
				line = location.line;
				column = location.column;
			}
		}
		if ((quote === '"' || quote === "'") && looksLikeTerminalPath(pathText)) {
			matches.push({
				column,
				index: quoted.index,
				line,
				original: quoted[0],
				pathText,
				quoted: quote === '"' ? 'double' : 'single',
			});
		}
		quoted = QUOTED_PATH_REGEX.exec(text);
	}
	collectUnquotedPathMatches(text, SPACED_FILE_PATH_REGEX, matches);
	collectUnquotedPathMatches(text, SHELL_ESCAPED_PATH_REGEX, matches);
	collectUnquotedPathMatches(text, PATH_REGEX, matches);
	return matches
		.filter(
			(candidate, index, all) =>
				all.findIndex(
					(other) =>
						candidate.index >= other.index &&
						candidate.index + candidate.original.length <= other.index + other.original.length,
				) === index,
		)
		.sort((left, right) => left.index - right.index);
}

function collectUnquotedPathMatches(text: string, regex: RegExp, matches: PathTextMatch[]): void {
	regex.lastIndex = 0;
	let match: RegExpExecArray | null = regex.exec(text);
	while (match !== null) {
		const original = stripTrailingPathPunctuation(match[0]);
		const suffixLength =
			(match[1] ? match[1].length + 1 : 0) + (match[2] ? match[2].length + 1 : 0);
		const pathText = original.slice(0, Math.max(0, original.length - suffixLength));
		if (pathText) {
			matches.push({
				column: parseOptionalPositiveInteger(match[2]),
				index: match.index,
				line: parseOptionalPositiveInteger(match[1]),
				original,
				pathText,
				quoted: 'unquoted',
			});
		}
		match = regex.exec(text);
	}
}

function looksLikeTerminalPath(value: string): boolean {
	return (
		value.startsWith('file://') ||
		value.startsWith('/') ||
		value.startsWith('~/') ||
		value.startsWith('./') ||
		value.startsWith('../') ||
		isWindowsAbsolutePath(value) ||
		value.includes('/') ||
		value.includes('\\') ||
		BARE_FILE_REGEX.test(value)
	);
}

const BARE_FILE_REGEX = new RegExp(String.raw`^${BARE_FILE}$`, 'u');

function decodeDisplayedPath(value: string, quoted: PathTextMatch['quoted']): string {
	if (quoted === 'single' || isWindowsAbsolutePath(value)) return value;
	return value.replaceAll(/\\([ \\"'])/gu, '$1');
}

function parseOptionalPositiveInteger(value: string | undefined): number | undefined {
	if (value === undefined) return undefined;
	const parsed = Number(value);
	return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : undefined;
}

function terminalPathLocation(
	value: string,
): { readonly column: number | undefined; readonly line: number; readonly path: string } | null {
	const match = value.match(/^(.*?):(\d+)(?::(\d+))?$/u);
	const path = match?.[1];
	const line = parseOptionalPositiveInteger(match?.[2]);
	if (path === undefined || line === undefined || !looksLikeTerminalPath(path)) return null;
	return { column: parseOptionalPositiveInteger(match?.[3]), line, path };
}

function positionsTouch(
	previous: { readonly x: number; readonly y: number },
	next: { readonly x: number; readonly y: number },
	rows: readonly TerminalBufferRow[],
): boolean {
	if (previous.y === next.y) return next.x === previous.x + 1 || next.x === previous.x + 2;
	const previousRow = rows.find((row) => Number(row.index) === previous.y);
	return next.y === previous.y + 1 && previousRow?.wrap === true;
}

function overlapsAny(range: ILinkRange, links: readonly ILink[]): boolean {
	return links.some(
		(link) =>
			terminalLinkContains(link, range.start.x, range.start.y) ||
			terminalLinkContains(link, range.end.x, range.end.y) ||
			terminalLinkContains({ range, text: '' }, link.range.start.x, link.range.start.y),
	);
}

function resolvePath(path: string, currentDirectory: string): string {
	if (path.startsWith('file://')) {
		return fileUrlPath(path) ?? path;
	}
	if (
		path.startsWith('/') ||
		path.startsWith('~') ||
		isWindowsAbsolutePath(path) ||
		!currentDirectory
	) {
		return normalizePath(path);
	}
	return normalizePath(`${normalizeCurrentDirectory(currentDirectory)}/${path}`);
}

function normalizeCurrentDirectory(currentDirectory: string): string {
	if (!currentDirectory.startsWith('file://')) return currentDirectory;
	return fileUrlPath(currentDirectory) ?? currentDirectory;
}

function normalizePath(path: string): string {
	const unified = path.replaceAll('\\', '/');
	const drive = unified.match(/^([A-Za-z]:)\//u)?.[1];
	const absolute = unified.startsWith('/') || drive !== undefined;
	const remainder = drive ? unified.slice(drive.length + 1) : unified;
	const parts: string[] = [];
	for (const part of remainder.split('/')) {
		if (!part || part === '.') continue;
		if (part === '..' && parts.length > 0 && parts.at(-1) !== '..') parts.pop();
		else if (part !== '..' || !absolute) parts.push(part);
	}
	let prefix = '';
	if (drive !== undefined) prefix = `${drive}/`;
	else if (absolute) prefix = '/';
	const normalized = `${prefix}${parts.join('/')}`;
	if (normalized) return normalized;
	if (drive) return `${drive}/`;
	return absolute ? '/' : '.';
}

function isWindowsAbsolutePath(path: string): boolean {
	return /^[A-Za-z]:[\\/]/u.test(path);
}

function normalizeFileUrlPath(path: string): string {
	return /^\/[A-Za-z]:\//u.test(path) ? path.slice(1) : path;
}

function stripTrailingPathPunctuation(path: string): string {
	if (/(?:^|[\\/])\.{1,2}$/u.test(path) || path.endsWith('..')) return path;
	return path.replace(TRAILING_PATH_PUNCTUATION, '');
}

function explicitTerminalTarget(target: string, currentDirectory: string): TerminalOpenTarget {
	if (!target.startsWith('file:')) return { kind: 'url', url: target };
	const path = fileUrlPath(target);
	return path === null
		? { kind: 'unknown', value: target }
		: { kind: 'file', path: resolvePath(path, currentDirectory) };
}

function fileUrlPath(value: string): string | null {
	try {
		const url = new URL(value);
		if (url.protocol !== 'file:' || (url.hostname !== '' && url.hostname !== 'localhost'))
			return null;
		return normalizeFileUrlPath(decodeURIComponent(url.pathname));
	} catch {
		return null;
	}
}
