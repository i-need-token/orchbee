import { TerminalProfileSchema, type TerminalProfile } from '@orchbee/terminal-protocol';

const ANSI_COLORS = [
	0x00_00_00, 0xcd_31_31, 0x10_7c_10, 0x94_98_00, 0x04_51_a5, 0xbc_05_bc, 0x05_98_bc, 0x55_55_55,
	0x66_66_66, 0xcd_31_31, 0x14_ce_14, 0xb5_ba_00, 0x04_51_a5, 0xbc_05_bc, 0x05_98_bc, 0xa5_a5_a5,
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
		colorScheme: 'light',
		defaultBackgroundRgb: 0xfc_fc_fc,
		defaultCursorRgb: null,
		defaultForegroundRgb: 0x14_14_14,
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
