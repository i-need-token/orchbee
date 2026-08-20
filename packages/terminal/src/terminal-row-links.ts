import type { TerminalBufferRow } from './buffer-rows';
import { normalizePermittedExternalTerminalUrl } from './normalizePermittedExternalTerminalUrl';
import type { ILink, ILinkRange } from './types';

const PATH_COMPONENT = String.raw`[\p{L}\p{N}\p{M}_.@%+,-]+`;
const PATH_SEPARATOR = String.raw`[\\/]`;
const PATH_REGEX = new RegExp(
	String.raw`(?:file:\/\/(?:[A-Za-z]:|${PATH_COMPONENT})?(?:${PATH_SEPARATOR}${PATH_COMPONENT})+|[A-Za-z]:${PATH_SEPARATOR}(?:${PATH_COMPONENT}${PATH_SEPARATOR})*${PATH_COMPONENT}|(?:~|\.{1,2})?${PATH_SEPARATOR}(?:${PATH_COMPONENT}${PATH_SEPARATOR})*${PATH_COMPONENT}|(?:${PATH_COMPONENT}${PATH_SEPARATOR})+${PATH_COMPONENT})(?::(\d+)(?::(\d+))?)?`,
	'gu',
);
const TRAILING_PATH_PUNCTUATION = /[.,;!?)\]，。！？）】]+$/u;
const TRAILING_URL_PUNCTUATION = /[.,;!?)\]]+$/u;
const URL_REGEX =
	/(?:https?:\/\/|mailto:|ftp:\/\/|ssh:\/\/|git:\/\/|tel:|magnet:|gemini:\/\/|gopher:\/\/|news:)[\w\-.~:/?#@!$&*+,;=%]+/giu;

interface LogicalTerminalText {
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
	const explicit = detectExplicitLinks(rows);
	const logicalLines = buildLogicalTexts(rows);
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

function buildLogicalTexts(rows: readonly TerminalBufferRow[]): readonly LogicalTerminalText[] {
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

function detectExplicitLinks(rows: readonly TerminalBufferRow[]): readonly ILink[] {
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
			target: { kind: 'url', url: active.target },
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
	PATH_REGEX.lastIndex = 0;
	let match: RegExpExecArray | null = PATH_REGEX.exec(logical.text);
	while (match !== null) {
		const original = stripTrailingPathPunctuation(match[0]);
		const suffixLength =
			(match[1] ? match[1].length + 1 : 0) + (match[2] ? match[2].length + 1 : 0);
		const pathText = original.slice(0, Math.max(0, original.length - suffixLength));
		const start = logical.positions[match.index];
		const end = logical.positions[match.index + original.length - 1];
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
						...(match[1] ? { line: Number(match[1]) } : {}),
						...(match[2] ? { column: Number(match[2]) } : {}),
					},
					text: original,
				});
			}
		}
		match = PATH_REGEX.exec(logical.text);
	}
	return links;
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
		try {
			return normalizeFileUrlPath(decodeURIComponent(new URL(path).pathname));
		} catch {
			return path;
		}
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
	try {
		return normalizeFileUrlPath(decodeURIComponent(new URL(currentDirectory).pathname));
	} catch {
		return currentDirectory;
	}
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
