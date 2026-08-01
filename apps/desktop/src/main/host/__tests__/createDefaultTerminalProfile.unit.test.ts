import { describe, expect, it } from 'vitest';

import { createDefaultTerminalProfile } from '../createDefaultTerminalProfile';

describe('createDefaultTerminalProfile', () => {
	it('builds the complete ANSI, color-cube, and grayscale palette', () => {
		const profile = createDefaultTerminalProfile();

		expect(profile.palette).toHaveLength(256);
		expect(profile.palette.slice(0, 3)).toEqual([0x00_00_00, 0xcd_31_31, 0x0d_bc_79]);
		expect(profile.palette[16]).toBe(0);
		expect(profile.palette[231]).toBe(0xff_ff_ff);
		expect(profile.palette[232]).toBe(0x08_08_08);
		expect(profile.palette[255]).toBe(0xee_ee_ee);
		expect(profile.versionResponse).toBe('Orchbee');
	});
});
