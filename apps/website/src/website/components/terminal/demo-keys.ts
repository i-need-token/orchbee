// Keys the key demo lists until the visitor presses their own, with the Kitty keyboard
// protocol sequence a program receives for each.
export const DEMO_KEYS = [
	{ name: 'Ctrl + Shift + Enter', sequence: '\\e[13;6u' },
	{ name: 'Alt + ←', sequence: '\\e[1;3D' },
	{ name: 'Esc', sequence: '\\e[27u' },
] as const;
