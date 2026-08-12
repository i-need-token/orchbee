import { MAXIMUM_PROJECTED_TERMINAL_PASTE_PREVIEW_CHARACTERS } from '../constants/projected-terminal-paste-review';

export function projectedTerminalPastePreview(text: string): {
	readonly text: string;
	readonly truncated: boolean;
} {
	let preview = '';
	let characters = 0;
	for (const character of text) {
		if (characters === MAXIMUM_PROJECTED_TERMINAL_PASTE_PREVIEW_CHARACTERS) {
			return { text: preview, truncated: true };
		}
		characters += 1;
		const codepoint = character.codePointAt(0) ?? 0;
		if (character === '\n') preview += character;
		else if (character === '\r') preview += '\\r';
		else if (character === '\t') preview += '\\t';
		else if (codepoint < 0x20 || (codepoint >= 0x7f && codepoint <= 0x9f)) {
			preview += `\\u${codepoint.toString(16).toUpperCase().padStart(4, '0')}`;
		} else preview += character;
	}
	return { text: preview, truncated: false };
}
