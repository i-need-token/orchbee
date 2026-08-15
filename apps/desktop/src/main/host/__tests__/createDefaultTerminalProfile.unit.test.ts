import { describe, expect, it } from 'vitest';

import { createDefaultTerminalProfile } from '../createDefaultTerminalProfile';

describe('createDefaultTerminalProfile', () => {
	it('builds the complete ANSI, color-cube, and grayscale palette', () => {
		const profile = createDefaultTerminalProfile();

		expect(profile.colorScheme).toBe('light');
		expect(profile.defaultBackgroundRgb).toBe(0xfc_fc_fc);
		expect(profile.defaultCursorRgb).toBe(0x57_be_f8);
		expect(profile.defaultForegroundRgb).toBe(0x14_14_14);
		expect(profile.palette).toHaveLength(256);
		expect(profile.palette.slice(0, 16)).toEqual([
			0x00_00_00, 0xcd_31_31, 0x10_7c_10, 0x94_98_00, 0x04_51_a5, 0xbc_05_bc, 0x05_98_bc,
			0x55_55_55, 0x66_66_66, 0xcd_31_31, 0x14_ce_14, 0xb5_ba_00, 0x04_51_a5, 0xbc_05_bc,
			0x05_98_bc, 0xa5_a5_a5,
		]);
		expect(profile.palette[16]).toBe(0);
		expect(profile.palette[231]).toBe(0xff_ff_ff);
		expect(profile.palette[232]).toBe(0x08_08_08);
		expect(profile.palette[255]).toBe(0xee_ee_ee);
		expect(profile.versionResponse).toBe('Orchbee');
	});
});
