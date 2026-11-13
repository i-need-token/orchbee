export interface KittyKey {
	readonly name: string;
	readonly sequence: string;
}

type ModifierKey = 'Shift' | 'Control' | 'Alt' | 'Meta';

const MODIFIERS: readonly (readonly [
	flag: 'shiftKey' | 'altKey' | 'ctrlKey' | 'metaKey',
	key: ModifierKey,
])[] = [
	['ctrlKey', 'Control'],
	['altKey', 'Alt'],
	['shiftKey', 'Shift'],
	['metaKey', 'Meta'],
];
const MODIFIER_BITS: Readonly<Record<ModifierKey, number>> = {
	Shift: 1,
	Alt: 2,
	Control: 4,
	Meta: 8,
};
const MODIFIER_CODES: Readonly<Record<ModifierKey, number>> = {
	Shift: 57441,
	Control: 57442,
	Alt: 57443,
	Meta: 57444,
};
const CODEPOINT_KEYS: Readonly<Record<string, number>> = {
	Enter: 13,
	Escape: 27,
	Backspace: 127,
	' ': 32,
};
const LETTER_KEYS: Readonly<Record<string, string>> = {
	ArrowUp: 'A',
	ArrowDown: 'B',
	ArrowRight: 'C',
	ArrowLeft: 'D',
	Home: 'H',
	End: 'F',
};
const TILDE_KEYS: Readonly<Record<string, number>> = {
	Insert: 2,
	Delete: 3,
	PageUp: 5,
	PageDown: 6,
	F3: 13,
	F5: 15,
	F6: 17,
	F7: 18,
	F8: 19,
	F9: 20,
	F10: 21,
	F11: 23,
	F12: 24,
};
const SS3_KEYS: Readonly<Record<string, string>> = { F1: 'P', F2: 'Q', F4: 'S' };
const KEY_NAMES: Readonly<Record<string, string>> = {
	ArrowUp: '↑',
	ArrowDown: '↓',
	ArrowRight: '→',
	ArrowLeft: '←',
	Escape: 'Esc',
	' ': 'Space',
	PageUp: 'PgUp',
	PageDown: 'PgDn',
	Delete: 'Del',
	Insert: 'Ins',
};
const IGNORED_KEYS = new Set(['', 'Tab', 'Unidentified', 'Process', 'Dead']);
const CSI = '\\e[';

function isModifierKey(key: string): key is ModifierKey {
	return key in MODIFIER_BITS;
}

/**
 * The key a program receives under the Kitty keyboard protocol for a browser key event, written
 * the way a person reads it, or undefined for keys the demo leaves to the browser.
 */
export function encodeKittyKey(event: KeyboardEvent, mac: boolean): KittyKey | undefined {
	const { key } = event;
	if (IGNORED_KEYS.has(key)) return undefined;
	const modifierName = (modifier: ModifierKey): string =>
		({ Shift: 'Shift', Control: 'Ctrl', Alt: mac ? 'Option' : 'Alt', Meta: mac ? 'Cmd' : 'Super' })[
			modifier
		];
	let bits = 0;
	const held: string[] = [];
	for (const [flag, modifier] of MODIFIERS) {
		if (!event[flag]) continue;
		bits |= MODIFIER_BITS[modifier];
		if (key !== modifier) held.push(modifierName(modifier));
	}
	const suffix = bits === 0 ? '' : `;${bits + 1}`;
	let name = KEY_NAMES[key] ?? key;
	let sequence: string;
	const codepoint = CODEPOINT_KEYS[key];
	const letter = LETTER_KEYS[key];
	const tilde = TILDE_KEYS[key];
	const ss3 = SS3_KEYS[key];
	if (isModifierKey(key)) {
		name = modifierName(key);
		sequence = `${CSI}${MODIFIER_CODES[key]};${bits + 1}u`;
	} else if (codepoint !== undefined) sequence = `${CSI}${codepoint}${suffix}u`;
	else if (letter !== undefined) sequence = `${CSI}${suffix === '' ? '' : `1${suffix}`}${letter}`;
	else if (tilde !== undefined) sequence = `${CSI}${tilde}${suffix}~`;
	else if (ss3 !== undefined) sequence = `${CSI}${suffix === '' ? '' : `1${suffix}`}${ss3}`;
	else {
		const { code } = event;
		let base = '';
		if (/^Key[A-Z]$/u.test(code)) base = code.slice(3).toLowerCase();
		else if (/^Digit\d$/u.test(code)) base = code.slice(5);
		else if ([...key].length === 1) base = key.toLowerCase();
		const point = base.codePointAt(0);
		if (point === undefined) return undefined;
		sequence = `${CSI}${point}${suffix}u`;
		name = /^[a-z]$/u.test(base) ? base.toUpperCase() : base;
	}
	return { name: [...held, name].join(' + '), sequence };
}
