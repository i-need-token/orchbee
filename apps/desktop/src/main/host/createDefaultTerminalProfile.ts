import { TerminalProfileSchema, type TerminalProfile } from '@orchbee/terminal-protocol';

const ANSI_COLORS = [
	0x00_00_00, 0xcd_31_31, 0x0d_bc_79, 0xe5_e5_10, 0x24_72_c8, 0xbc_3f_bc, 0x11_a8_cd, 0xe5_e5_e5,
	0x66_66_66, 0xf1_4c_4c, 0x23_d1_8b, 0xf5_f5_43, 0x3b_8e_ea, 0xd6_70_d6, 0x29_b8_db, 0xff_ff_ff,
] as const;
const COLOR_CUBE_STEPS = [0, 95, 135, 175, 215, 255] as const;

export function createDefaultTerminalProfile(): TerminalProfile {
	const palette: number[] = [...ANSI_COLORS];
	for (const red of COLOR_CUBE_STEPS) {
		for (const green of COLOR_CUBE_STEPS) {
			for (const blue of COLOR_CUBE_STEPS) palette.push((red << 16) | (green << 8) | blue);
		}
	}
	for (let index = 0; index < 24; index += 1) {
		const value = 8 + index * 10;
		palette.push((value << 16) | (value << 8) | value);
	}

	return TerminalProfileSchema.parse({
		colorScheme: 'dark',
		defaultBackgroundRgb: 0x1e_1e_1e,
		defaultCursorRgb: null,
		defaultForegroundRgb: 0xd4_d4_d4,
		deviceAttributes: {
			primaryConformanceLevel: 62,
			primaryFeatures: [22],
			secondaryDeviceType: 1,
			secondaryFirmwareVersion: 0,
			secondaryRomCartridge: 0,
			tertiaryUnitId: 0,
		},
		enquiryResponse: '',
		palette,
		versionResponse: 'Orchbee',
	});
}
