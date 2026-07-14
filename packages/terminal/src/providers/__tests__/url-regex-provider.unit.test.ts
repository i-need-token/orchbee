import { describe, expect, it } from 'vitest';
import { UrlRegexProvider } from '../url-regex-provider';
import type { ILink, ILinkBufferLine } from '../../types';

function lineFrom(text: string, isWrapped = false): ILinkBufferLine {
	return {
		length: text.length,
		isWrapped,
		getCell: (index: number) => ({
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

describe('UrlRegexProvider', () => {
	it('detects URLs and strips trailing punctuation', () => {
		const text = 'Visit https://example.com/path).';
		const provider = new UrlRegexProvider({
			buffer: { active: { length: 3, getLine: () => lineFrom(text) } },
		});
		let links: ILink[] | undefined;
		provider.provideLinks(2, (value) => {
			links = value;
		});
		expect(links?.[0]).toMatchObject({
			text: 'https://example.com/path',
			range: { start: { x: 6, y: 2 }, end: { x: 29, y: 2 } },
			target: { kind: 'url', url: 'https://example.com/path' },
			activation: 'modifier',
		});
	});

	it('detects one URL across a soft-wrapped logical line', () => {
		const lines = [lineFrom('prefix https://example.', true), lineFrom('com/path suffix')];
		const provider = new UrlRegexProvider({
			buffer: { active: { length: lines.length, getLine: (row) => lines[row] } },
		});
		let links: ILink[] | undefined;
		provider.provideLinks(1, (value) => {
			links = value;
		});
		expect(links?.[0]).toMatchObject({
			text: 'https://example.com/path',
			range: { start: { x: 7, y: 0 }, end: { x: 7, y: 1 } },
			target: { kind: 'url', url: 'https://example.com/path' },
		});
	});
});
