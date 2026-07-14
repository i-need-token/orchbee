import type { ILink, ILinkBuffer, ILinkBufferLine, ILinkProvider } from '../types';

export interface ITerminalForPathProvider {
	readonly buffer: { readonly active: ILinkBuffer };
	getPwd(): string;
	getSemanticZone(row: number, column: number): 'prompt' | 'input' | 'output' | null;
}

const PATH_REGEX =
	/(?:file:\/\/[^\s'"<>]+|(?:~|\.{1,2})?\/[\w.@%+,-]+(?:\/[\w.@%+,-]+)*|(?:[\w.@%+,-]+\/)+[\w.@%+,-]+)(?::(\d+)(?::(\d+))?)?/g;
const TRAILING_PUNCTUATION = /[.,;!?)\]]+$/;

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
			const original = match[0].replace(TRAILING_PUNCTUATION, '');
			const suffixLength =
				(match[1] ? match[1].length + 1 : 0) + (match[2] ? match[2].length + 1 : 0);
			const pathText = original.slice(0, Math.max(0, original.length - suffixLength));
			const start = logicalLine.positions[match.index];
			const end = logicalLine.positions[match.index + original.length - 1];
			if (pathText && start && end) {
				const semanticZone = this.terminal.getSemanticZone(start.y, start.x);
				if (semanticZone !== 'prompt' || pathText.startsWith('/') || pathText.startsWith('~')) {
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
			return decodeURIComponent(new URL(path).pathname);
		} catch {
			return path;
		}
	}
	if (path.startsWith('/') || path.startsWith('~') || !pwd) return normalizePath(path);
	return normalizePath(`${normalizePwd(pwd)}/${path}`);
}

function normalizePwd(pwd: string): string {
	if (!pwd.startsWith('file://')) return pwd;
	try {
		return decodeURIComponent(new URL(pwd).pathname);
	} catch {
		return pwd;
	}
}

function normalizePath(path: string): string {
	const absolute = path.startsWith('/');
	const parts: string[] = [];
	for (const part of path.split('/')) {
		if (!part || part === '.') continue;
		if (part === '..' && parts.length > 0 && parts.at(-1) !== '..') parts.pop();
		else if (part !== '..' || !absolute) parts.push(part);
	}
	return `${absolute ? '/' : ''}${parts.join('/')}` || (absolute ? '/' : '.');
}
