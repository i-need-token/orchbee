import { RowPromptClass, SemanticContent } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import type { TerminalBufferRenderCell, TerminalBufferRow } from '../buffer-rows';
import { detectTerminalRowLinks, isPermittedExternalTerminalTarget } from '../terminal-row-links';

describe('source-neutral terminal row links', () => {
	it('gives an overlapping OSC 8 target precedence over plain URL detection', () => {
		const rows = [row(4n, 'https://plain.example', 'https://explicit.example')];
		const links = detectTerminalRowLinks(rows);

		expect(links).toHaveLength(1);
		expect(links[0]?.target).toEqual({ kind: 'url', url: 'https://explicit.example' });
		expect(isPermittedExternalTerminalTarget(links[0] ?? unreachableLink())).toBe(true);
	});

	it('detects one URL across a soft wrap using absolute row coordinates', () => {
		const links = detectTerminalRowLinks([
			row(8n, 'https://example.', null, true),
			row(9n, 'com/path', null, false, true),
		]);

		expect(links).toEqual([
			expect.objectContaining({
				range: { end: { x: 7, y: 9 }, start: { x: 0, y: 8 } },
				target: { kind: 'url', url: 'https://example.com/path' },
			}),
		]);
	});

	it('auto-detects only the approved external URL schemes', () => {
		const links = detectTerminalRowLinks([
			row(
				10n,
				[
					'http://http.example',
					'https://https.example',
					'mailto:terminal@orchbee.dev',
					'ftp://ftp.example',
					'ssh://ssh.example',
					'tel:+15551234567',
				].join(' '),
			),
		]);

		expect(
			links
				.filter((link) => link.target?.kind === 'url')
				.map((link) => (link.target?.kind === 'url' ? link.target.url : 'unreachable')),
		).toEqual(['http://http.example', 'https://https.example', 'mailto:terminal@orchbee.dev']);
		expect(
			links.every((link) => link.target?.kind !== 'url' || isPermittedExternalTerminalTarget(link)),
		).toBe(true);
	});

	it('retains unsupported explicit metadata without making it activatable', () => {
		const links = detectTerminalRowLinks([row(11n, 'explicit ftp', 'ftp://ftp.example/path')]);

		expect(links).toEqual([
			expect.objectContaining({
				target: { kind: 'url', url: 'ftp://ftp.example/path' },
			}),
		]);
		expect(isPermittedExternalTerminalTarget(links[0] ?? unreachableLink())).toBe(false);
	});

	it('keeps detected paths inactive under the external-navigation policy', () => {
		const links = detectTerminalRowLinks([row(2n, './src/index.ts:12:4')], '/workspace');
		const file = links.find((link) => link.target?.kind === 'file');

		expect(file?.target).toEqual({
			column: 4,
			kind: 'file',
			line: 12,
			path: '/workspace/src/index.ts',
		});
		expect(isPermittedExternalTerminalTarget(file ?? unreachableLink())).toBe(false);
	});

	it('detects a bare filename with a common extension', () => {
		const links = detectTerminalRowLinks([row(3n, 'README.md')], '/workspace');

		expect(links).toEqual([
			expect.objectContaining({
				target: { kind: 'file', path: '/workspace/README.md' },
				text: 'README.md',
			}),
		]);
	});

	it('keeps adjacent bare and nested file paths as separate targets', () => {
		const links = detectTerminalRowLinks([
			row(3n, 'https://orchbee.dev README.md src/main.ts:42:7 deadbee'),
		]);

		expect(links.map((link) => link.target)).toEqual([
			{ kind: 'url', url: 'https://orchbee.dev' },
			{ kind: 'file', path: 'README.md' },
			{ column: 7, kind: 'file', line: 42, path: 'src/main.ts' },
		]);
	});

	it('detects quoted and unquoted image paths containing spaces', () => {
		const links = detectTerminalRowLinks(
			[
				row(
					3n,
					`'/tmp/Agent Screens/final image.png' ./art/reference image.webp "../shots/draft image.jpg"`,
				),
			],
			'/workspace/project',
		);

		expect(
			links
				.filter((link) => link.target?.kind === 'file')
				.map((link) => (link.target?.kind === 'file' ? link.target.path : 'unreachable')),
		).toEqual([
			'/tmp/Agent Screens/final image.png',
			'/workspace/project/art/reference image.webp',
			'/workspace/shots/draft image.jpg',
		]);
	});

	it('decodes shell-escaped spaces before resolving a relative path', () => {
		const links = detectTerminalRowLinks(
			[row(3n, String.raw`./art/final\ image.jpeg:9:2`)],
			'/workspace',
		);
		const file = links.find((link) => link.target?.kind === 'file');

		expect(file).toEqual(
			expect.objectContaining({
				target: {
					column: 2,
					kind: 'file',
					line: 9,
					path: '/workspace/art/final image.jpeg',
				},
				text: String.raw`./art/final\ image.jpeg:9:2`,
			}),
		);
	});

	it('detects quoted, ordinary, and shell-escaped directory paths', () => {
		const links = detectTerminalRowLinks(
			[row(3n, String.raw`'/tmp/Agent Screens' ./build/output /tmp/Agent\ Screens`)],
			'/workspace',
		);

		expect(links.map((link) => link.target)).toEqual([
			{ kind: 'file', path: '/tmp/Agent Screens' },
			{ kind: 'file', path: '/workspace/build/output' },
			{ kind: 'file', path: '/tmp/Agent Screens' },
		]);
	});

	it('extracts line and column suffixes placed inside or after path quotes', () => {
		const links = detectTerminalRowLinks(
			[row(4n, `"./art/final image.png:12:3" '/tmp/other image.jpg':9:2`)],
			'/workspace',
		);

		expect(links.map((link) => link.target)).toEqual([
			{ column: 3, kind: 'file', line: 12, path: '/workspace/art/final image.png' },
			{ column: 2, kind: 'file', line: 9, path: '/tmp/other image.jpg' },
		]);
	});

	it('classifies an explicit local file URL as a decoded file target', () => {
		const links = detectTerminalRowLinks([
			row(5n, 'generated preview', 'file:///tmp/agent%20shots/result.PNG'),
		]);

		expect(links).toEqual([
			expect.objectContaining({
				target: { kind: 'file', path: '/tmp/agent shots/result.PNG' },
			}),
		]);
	});

	it('maps URLs in implicit narrow ASCII rows', () => {
		const text = 'https://example.com/path';
		const links = detectTerminalRowLinks([
			{
				cells: null,
				identityAscii: true,
				index: 12n,
				mappings: [],
				promptClass: RowPromptClass.none,
				text,
				wrap: false,
				wrapContinuation: false,
			},
		]);

		expect(links).toEqual([
			expect.objectContaining({
				range: { end: { x: text.length - 1, y: 12 }, start: { x: 0, y: 12 } },
				target: { kind: 'url', url: text },
			}),
		]);
	});
});

function row(
	index: bigint,
	text: string,
	hyperlink: string | null = null,
	wrap = false,
	wrapContinuation = false,
): TerminalBufferRow {
	const cells: TerminalBufferRenderCell[] = Array.from(text, (grapheme, column) => ({
		column,
		grapheme,
		hyperlink,
		semanticContent: SemanticContent.output,
		width: 1,
	}));
	return {
		cells,
		index,
		mappings: Array.from(text, (_character, column) => ({
			byteOffset: column,
			column,
			utf16Offset: column,
			width: 1,
		})),
		promptClass: RowPromptClass.none,
		text,
		wrap,
		wrapContinuation,
	};
}

function unreachableLink(): never {
	throw new Error('Expected a detected link');
}
