import { describe, expect, it } from 'vitest';

import { parseScrollbackMegabytes } from '../terminal-settings-draft-parsing';

describe('terminal settings draft parsing', () => {
	it('admits plain decimal terminal history within the Host range', () => {
		expect(parseScrollbackMegabytes('0')).toBe(0);
		expect(parseScrollbackMegabytes('10')).toBe(10_000_000);
		expect(parseScrollbackMegabytes('2.5')).toBe(2_500_000);
		expect(parseScrollbackMegabytes('1000')).toBe(1_000_000_000);
		expect(parseScrollbackMegabytes('01')).toBeNull();
		expect(parseScrollbackMegabytes('-1')).toBeNull();
		expect(parseScrollbackMegabytes('1000.1')).toBeNull();
	});
});
