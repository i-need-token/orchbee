import { terminalColorThemes, type TerminalColorTheme } from '@orchbee/terminal';

/** A test-only named theme pair; the build ships only the Orchbee pair. */
export const fixtureDarkTheme: TerminalColorTheme = Object.freeze({
	id: 'fixture-dusk',
	family: 'fixture',
	polarity: 'dark',
	background: 0x2e_34_40,
	foreground: 0xd8_de_e9,
	cursor: 0x88_c0_d0,
	cursorText: 0x2e_34_40,
	selectionBackground: 0x43_4c_5e,
	selectionForeground: 0xec_ef_f4,
	ansi: Object.freeze({
		black: 0x3b_42_52,
		red: 0xbf_61_6a,
		green: 0xa3_be_8c,
		yellow: 0xeb_cb_8b,
		blue: 0x81_a1_c1,
		magenta: 0xb4_8e_ad,
		cyan: 0x88_c0_d0,
		white: 0xe5_e9_f0,
		brightBlack: 0x4c_56_6a,
		brightRed: 0xbf_61_6b,
		brightGreen: 0xa3_be_8d,
		brightYellow: 0xeb_cb_8c,
		brightBlue: 0x81_a1_c2,
		brightMagenta: 0xb4_8e_ae,
		brightCyan: 0x8f_bc_bb,
		brightWhite: 0xec_ef_f4,
	}),
});

export const fixtureLightTheme: TerminalColorTheme = Object.freeze({
	id: 'fixture-dawn',
	family: 'fixture',
	polarity: 'light',
	background: 0xfd_f6_e3,
	foreground: 0x58_6e_75,
	cursor: 0x26_8b_d2,
	cursorText: 0xfd_f6_e3,
	selectionBackground: 0xee_e8_d5,
	selectionForeground: 0x07_36_42,
	ansi: Object.freeze({
		black: 0x07_36_42,
		red: 0xdc_32_2f,
		green: 0x85_99_00,
		yellow: 0xb5_89_00,
		blue: 0x26_8b_d2,
		magenta: 0xd3_36_82,
		cyan: 0x2a_a1_98,
		white: 0xee_e8_d5,
		brightBlack: 0x00_2b_36,
		brightRed: 0xcb_4b_16,
		brightGreen: 0x58_6e_76,
		brightYellow: 0x65_7b_83,
		brightBlue: 0x83_94_96,
		brightMagenta: 0x6c_71_c4,
		brightCyan: 0x93_a1_a1,
		brightWhite: 0xfd_f6_e4,
	}),
});

/** The shipped themes plus the fixture pair, for surfaces that accept an injected catalog. */
export const colorThemesWithFixtures: readonly TerminalColorTheme[] = Object.freeze([
	...terminalColorThemes,
	fixtureLightTheme,
	fixtureDarkTheme,
]);
