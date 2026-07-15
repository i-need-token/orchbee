import { describe, expect, it } from 'vitest';
import { PathLinkProvider } from '../path-link-provider';
import type { ILink, ILinkBufferLine } from '../../types';

describe('PathLinkProvider', () => {
	it('resolves a relative path and source position against OSC PWD', () => {
		const provider = createProvider([line('error at src/main.ts:12:4')]);
		expect(provide(provider, 0)?.[0]).toMatchObject({
			text: 'src/main.ts:12:4',
			range: { start: { x: 9, y: 0 }, end: { x: 24, y: 0 } },
			target: {
				kind: 'file',
				path: '/Users/test/project/src/main.ts',
				line: 12,
				column: 4,
			},
		});
	});

	it('detects an absolute path across a soft wrap', () => {
		const provider = createProvider([line('open /Users/test/pro', true), line('ject/file.ts')]);
		expect(provide(provider, 1)?.[0]).toMatchObject({
			text: '/Users/test/project/file.ts',
			range: { start: { x: 5, y: 0 }, end: { x: 11, y: 1 } },
			target: { kind: 'file', path: '/Users/test/project/file.ts' },
		});
	});

	it('detects Unicode paths without splitting multibyte components', () => {
		const provider = createProvider([line('open /path/音楽/テストファイル.txt:16')]);
		expect(provide(provider, 0)?.[0]).toMatchObject({
			text: '/path/音楽/テストファイル.txt:16',
			target: {
				kind: 'file',
				path: '/path/音楽/テストファイル.txt',
				line: 16,
			},
		});
	});

	it('stops absolute paths at CJK sentence punctuation', () => {
		const provider = createProvider([line('see /a/b，/c/d。')]);
		expect(provide(provider, 0)).toMatchObject([
			{ text: '/a/b', target: { kind: 'file', path: '/a/b' } },
			{ text: '/c/d', target: { kind: 'file', path: '/c/d' } },
		]);
	});

	it('detects a path next to CJK prose and tree decorations', () => {
		const prose = createProvider([line('路径：/path/to/file.md')]);
		const tree = createProvider([line('│└──/tmp/音楽.md')]);
		expect(provide(prose, 0)?.[0]).toMatchObject({
			text: '/path/to/file.md',
			target: { kind: 'file', path: '/path/to/file.md' },
		});
		expect(provide(tree, 0)?.[0]).toMatchObject({
			text: '/tmp/音楽.md',
			target: { kind: 'file', path: '/tmp/音楽.md' },
		});
	});

	it('detects and normalizes Windows drive paths', () => {
		const provider = createProvider([
			line('C:/Users/c/warp-md-test.md. C:\\Users\\c\\other.ts:9:2'),
		]);
		expect(provide(provider, 0)).toMatchObject([
			{
				text: 'C:/Users/c/warp-md-test.md',
				target: { kind: 'file', path: 'C:/Users/c/warp-md-test.md' },
			},
			{
				text: 'C:\\Users\\c\\other.ts:9:2',
				target: {
					kind: 'file',
					path: 'C:/Users/c/other.ts',
					line: 9,
					column: 2,
				},
			},
		]);
	});

	it.each([
		{
			name: 'relative Markdown path followed by a sentence period',
			input: 'notes/README.md.',
			text: 'notes/README.md',
			path: '/Users/test/project/notes/README.md',
		},
		{
			name: 'absolute dotfile followed by a sentence period',
			input: '/tmp/.gitignore.',
			text: '/tmp/.gitignore',
			path: '/tmp/.gitignore',
		},
		{
			name: 'Windows forward-slash path followed by a sentence period',
			input: 'C:/Users/c/warp-md-test.md.',
			text: 'C:/Users/c/warp-md-test.md',
			path: 'C:/Users/c/warp-md-test.md',
		},
		{
			name: 'fullwidth comma after a path',
			input: '/tmp/README.md，',
			text: '/tmp/README.md',
			path: '/tmp/README.md',
		},
		{
			name: 'fullwidth sentence punctuation sequence after a path',
			input: '/tmp/README.md。！？',
			text: '/tmp/README.md',
			path: '/tmp/README.md',
		},
		{
			name: 'fullwidth parentheses around a path',
			input: '（/path/to/file）',
			text: '/path/to/file',
			path: '/path/to/file',
		},
		{
			name: 'CJK corner brackets around a path',
			input: '「/path/to/file」',
			text: '/path/to/file',
			path: '/path/to/file',
		},
		{
			name: 'fullwidth colon before a path',
			input: '路径：/path/to/file.md',
			text: '/path/to/file.md',
			path: '/path/to/file.md',
		},
		{
			name: 'tree decoration before an absolute Unicode path',
			input: '│└──/tmp/音楽.md',
			text: '/tmp/音楽.md',
			path: '/tmp/音楽.md',
		},
		{
			name: 'file URL with line and column',
			input: 'file:///tmp/main.ts:7:2',
			text: 'file:///tmp/main.ts:7:2',
			path: '/tmp/main.ts',
			line: 7,
			column: 2,
		},
		{
			name: 'current-directory relative path',
			input: './src/main.ts',
			text: './src/main.ts',
			path: '/Users/test/project/src/main.ts',
		},
		{
			name: 'parent-directory relative path',
			input: '../src/main.ts',
			text: '../src/main.ts',
			path: '/Users/test/src/main.ts',
		},
		{
			name: 'relative Windows separators',
			input: 'src\\nested\\main.ts',
			text: 'src\\nested\\main.ts',
			path: '/Users/test/project/src/nested/main.ts',
		},
		{
			name: 'dot path component',
			input: 'foo/.',
			text: 'foo/.',
			path: '/Users/test/project/foo',
		},
		{
			name: 'double-dot path component',
			input: 'foo/..',
			text: 'foo/..',
			path: '/Users/test/project',
		},
		{
			name: 'filename ending in two periods',
			input: '/tmp/foo..',
			text: '/tmp/foo..',
			path: '/tmp/foo..',
		},
	])('$name', ({ input, text, path, line: lineNumber, column }) => {
		const link = provide(createProvider([line(input)]), 0)?.[0];
		expect(link).toMatchObject({
			text,
			target: {
				kind: 'file',
				path,
				...(lineNumber === undefined ? {} : { line: lineNumber }),
				...(column === undefined ? {} : { column }),
			},
		});
	});

	it('keeps absolute Windows paths linkable in prompt text', () => {
		const provider = createProvider([line('C:\\Users\\c\\main.ts')], 'prompt');
		expect(provide(provider, 0)?.[0]).toMatchObject({
			text: 'C:\\Users\\c\\main.ts',
			target: { kind: 'file', path: 'C:/Users/c/main.ts' },
		});
	});
});

function createProvider(
	lines: readonly ILinkBufferLine[],
	semanticZone: 'prompt' | 'input' | 'output' | null = 'output',
): PathLinkProvider {
	return new PathLinkProvider({
		buffer: { active: { length: lines.length, getLine: (row) => lines[row] } },
		getPwd: () => 'file:///Users/test/project',
		getSemanticZone: () => semanticZone,
	});
}

function provide(provider: PathLinkProvider, row: number): ILink[] | undefined {
	let links: ILink[] | undefined;
	provider.provideLinks(row, (value) => {
		links = value;
	});
	return links;
}

function line(text: string, isWrapped = false): ILinkBufferLine {
	return {
		length: text.length,
		isWrapped,
		getCell: (index) => ({
			getCodepoint: () => text.codePointAt(index) ?? 0,
			getWidth: () => 1,
			getHyperlinkId: () => 0,
			isBold: () => 0,
			isItalic: () => 0,
			isDim: () => false,
		}),
		translateToString: () => text,
	};
}
