import type { ILink, ILinkBuffer, ILinkBufferLine, ILinkProvider } from '../types';

export interface ITerminalForPathProvider {
	readonly buffer: { readonly active: ILinkBuffer };
	getPwd(): string;
	getSemanticZone(row: number, column: number): 'prompt' | 'input' | 'output' | null;
}

const PATH_COMPONENT = String.raw`[\p{L}\p{N}\p{M}_.@%+,-]+`;
const PATH_SEPARATOR = String.raw`[\\/]`;
const PATH_REGEX = new RegExp(
	String.raw`(?:file:\/\/(?:[A-Za-z]:|${PATH_COMPONENT})?(?:${PATH_SEPARATOR}${PATH_COMPONENT})+|[A-Za-z]:${PATH_SEPARATOR}(?:${PATH_COMPONENT}${PATH_SEPARATOR})*${PATH_COMPONENT}|(?:~|\.{1,2})?${PATH_SEPARATOR}(?:${PATH_COMPONENT}${PATH_SEPARATOR})*${PATH_COMPONENT}|(?:${PATH_COMPONENT}${PATH_SEPARATOR})+${PATH_COMPONENT})(?::(\d+)(?::(\d+))?)?`,
	'gu',
);
const TRAILING_PUNCTUATION = /[.,;!?)\]，。！？）】]+$/u;

/** Detect file paths and resolve relative paths against OSC 7/9 PWD. */
export class PathLinkProvider implements ILinkProvider {
	private readonly terminal: ITerminalForPathProvider;

	constructor(terminal: ITerminalForPathProvider) {
		this.terminal = terminal;
	}

	provideLinks(y: number, callback: (links: ILink[] | undefined) => void): void {
		const logicalLine = this.readLogicalLine(y);
		if (!logicalLine) {
			callback(undefined);
			return;
		}
		const links: ILink[] = [];
		PATH_REGEX.lastIndex = 0;
		let match: RegExpExecArray | null = PATH_REGEX.exec(logicalLine.text);
		while (match) {
			const original = stripTrailingSentencePunctuation(match[0]);
			const suffixLength =
				(match[1] ? match[1].length + 1 : 0) + (match[2] ? match[2].length + 1 : 0);
			const pathText = original.slice(0, Math.max(0, original.length - suffixLength));
			const start = logicalLine.positions[match.index];
			const end = logicalLine.positions[match.index + original.length - 1];
			if (pathText && start && end) {
				const semanticZone = this.terminal.getSemanticZone(start.y, start.x);
				if (
					semanticZone !== 'prompt' ||
					pathText.startsWith('/') ||
					pathText.startsWith('~') ||
					isWindowsAbsolutePath(pathText)
				) {
					links.push({
						text: original,
						range: { start, end },
						target: {
							kind: 'file',
							path: resolvePath(pathText, this.terminal.getPwd()),
							...(match[1] ? { line: Number(match[1]) } : {}),
							...(match[2] ? { column: Number(match[2]) } : {}),
						},
						activation: 'modifier',
					});
				}
			}
			match = PATH_REGEX.exec(logicalLine.text);
		}
		callback(links.length ? links : undefined);
	}

	dispose(): void {}

	private readLogicalLine(y: number): {
		readonly text: string;
		readonly positions: readonly { readonly x: number; readonly y: number }[];
	} | null {
		const buffer = this.terminal.buffer.active;
		if (!buffer.getLine(y)) return null;
		let start = y;
		while (start > 0 && buffer.getLine(start - 1)?.isWrapped) start--;
		let text = '';
		const positions: Array<{ readonly x: number; readonly y: number }> = [];
		for (let row = start; row < buffer.length; row++) {
			const line = buffer.getLine(row);
			if (!line) break;
			const segment = readLine(line, row);
			text += segment.text;
			positions.push(...segment.positions);
			if (!line.isWrapped) break;
		}
		return { text, positions };
	}
}

function readLine(
	line: ILinkBufferLine,
	row: number,
): {
	readonly text: string;
	readonly positions: readonly { readonly x: number; readonly y: number }[];
} {
	let text = '';
	const positions: Array<{ readonly x: number; readonly y: number }> = [];
	for (let column = 0; column < line.length; column++) {
		const cell = line.getCell(column);
		if (!cell || cell.getWidth() === 0) continue;
		const codepoint = cell.getCodepoint();
		const value =
			codepoint === 0 || codepoint < 32 || codepoint > 0x10ffff
				? ' '
				: String.fromCodePoint(codepoint);
		text += value;
		for (let index = 0; index < value.length; index++) positions.push({ x: column, y: row });
	}
	return { text, positions };
}

function resolvePath(path: string, pwd: string): string {
	if (path.startsWith('file://')) {
		try {
			return normalizeFileUrlPath(decodeURIComponent(new URL(path).pathname));
		} catch {
			return path;
		}
	}
	if (path.startsWith('/') || path.startsWith('~') || isWindowsAbsolutePath(path) || !pwd) {
		return normalizePath(path);
	}
	return normalizePath(`${normalizePwd(pwd)}/${path}`);
}

function normalizePwd(pwd: string): string {
	if (!pwd.startsWith('file://')) return pwd;
	try {
		return normalizeFileUrlPath(decodeURIComponent(new URL(pwd).pathname));
	} catch {
		return pwd;
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
	if (drive) prefix = `${drive}/`;
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

function stripTrailingSentencePunctuation(path: string): string {
	if (/(?:^|[\\/])\.{1,2}$/u.test(path) || path.endsWith('..')) return path;
	return path.replace(TRAILING_PUNCTUATION, '');
}
