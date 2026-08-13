export const ORCHBEE_TERMINAL_FONT_FAMILY =
	'"Orchbee JetBrains Mono", "Orchbee Symbols Nerd Font", monospace';

const terminalFontFaces = [
	{
		descriptors: { style: 'normal', weight: '100 800' },
		family: 'Orchbee JetBrains Mono',
		source: new URL('./fonts/JetBrainsMono-Variable.ttf', import.meta.url),
	},
	{
		descriptors: { style: 'italic', weight: '100 800' },
		family: 'Orchbee JetBrains Mono',
		source: new URL('./fonts/JetBrainsMono-VariableItalic.ttf', import.meta.url),
	},
	{
		descriptors: { style: 'normal', weight: '400' },
		family: 'Orchbee Symbols Nerd Font',
		source: new URL('./fonts/SymbolsNerdFont-Regular.ttf', import.meta.url),
	},
] as const;

const terminalFontLicenses = [
	{
		id: 'jetbrains-mono',
		source: new URL('./fonts/JetBrainsMono-OFL.txt', import.meta.url),
	},
	{
		id: 'symbols-nerd-font',
		source: new URL('./fonts/SymbolsNerdFont-LICENSE.txt', import.meta.url),
	},
] as const;

export function registerTerminalProductFonts(document: Document): void {
	for (const face of terminalFontFaces) {
		document.fonts.add(
			new FontFace(face.family, `url("${face.source.href}") format("truetype")`, face.descriptors),
		);
	}
	for (const license of terminalFontLicenses) {
		if (
			document.head.querySelector(`link[data-orchbee-terminal-font-license="${license.id}"]`) !==
			null
		) {
			continue;
		}
		const link = document.createElement('link');
		link.dataset['orchbeeTerminalFontLicense'] = license.id;
		link.href = license.source.href;
		link.rel = 'license';
		document.head.append(link);
	}
}
