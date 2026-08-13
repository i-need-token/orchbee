import { afterEach, describe, expect, it, vi } from 'vitest';

import { registerTerminalProductFonts } from '../terminal-product-fonts';

class TestFontFace {
	readonly descriptors: FontFaceDescriptors;
	readonly family: string;
	readonly source: string;

	constructor(family: string, source: string, descriptors: FontFaceDescriptors) {
		this.descriptors = descriptors;
		this.family = family;
		this.source = source;
	}
}

describe('terminal product fonts', () => {
	afterEach(() => vi.unstubAllGlobals());

	it('registers the pinned Ghostty faces and exposes both licenses', () => {
		const add = vi.fn<(face: FontFace) => FontFaceSet>(() => ({}) as FontFaceSet);
		const licenses: Array<{ dataset: Record<string, string>; href: string; rel: string }> = [];
		const document = {
			createElement: () => ({ dataset: {}, href: '', rel: '' }),
			fonts: { add },
			head: {
				append: (link: { dataset: Record<string, string>; href: string; rel: string }) => {
					licenses.push(link);
				},
				querySelector: (selector: string) =>
					licenses.find((link) =>
						selector.includes(`="${link.dataset['orchbeeTerminalFontLicense']}"`),
					) ?? null,
			},
		} as unknown as Document;
		vi.stubGlobal('FontFace', TestFontFace);

		registerTerminalProductFonts(document);

		expect(add.mock.calls).toEqual([
			[
				expect.objectContaining({
					descriptors: { style: 'normal', weight: '100 800' },
					family: 'Orchbee JetBrains Mono',
					source: expect.stringContaining('JetBrainsMono-Variable.ttf'),
				}),
			],
			[
				expect.objectContaining({
					descriptors: { style: 'italic', weight: '100 800' },
					family: 'Orchbee JetBrains Mono',
					source: expect.stringContaining('JetBrainsMono-VariableItalic.ttf'),
				}),
			],
			[
				expect.objectContaining({
					descriptors: { style: 'normal', weight: '400' },
					family: 'Orchbee Symbols Nerd Font',
					source: expect.stringContaining('SymbolsNerdFont-Regular.ttf'),
				}),
			],
		]);
		expect(licenses.map((link) => link.href.slice(link.href.lastIndexOf('/') + 1))).toEqual([
			'JetBrainsMono-OFL.txt',
			'SymbolsNerdFont-LICENSE.txt',
		]);
	});
});
