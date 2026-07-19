import { describe, expect, it } from 'vitest';

import { resolveShortcutPlatform } from '../resolveShortcutPlatform';

describe('resolveShortcutPlatform', () => {
	it('maps Apple platform identifiers to the Apple shortcut modifier', () => {
		expect(resolveShortcutPlatform('MacIntel')).toBe('apple');
		expect(resolveShortcutPlatform('iPad')).toBe('apple');
	});

	it('maps other and missing platform identifiers to the standard modifier', () => {
		expect(resolveShortcutPlatform('Win32')).toBe('standard');
		expect(resolveShortcutPlatform('Linux x86_64')).toBe('standard');
		expect(resolveShortcutPlatform('')).toBe('standard');
	});
});
