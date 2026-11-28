import { describe, expect, it } from 'vitest';

import { formatDroppedPaths } from '../formatDroppedPaths';

// A pasted drop must reach the shell as the same paths, as the platform's own terminal types them.
describe('formatDroppedPaths', () => {
	it('quotes a Windows path only when it holds a space, as Windows Terminal does', () => {
		expect(formatDroppedPaths(['C:\\a\\b.txt', 'C:\\My Files\\项目'], 'windows')).toBe(
			'C:\\a\\b.txt "C:\\My Files\\项目"',
		);
	});

	it('escapes shell-special characters and ends each path with a space, as Terminal.app does', () => {
		expect(formatDroppedPaths(["/Users/me/My File (1)'s.txt", '/tmp/项目'], 'posix')).toBe(
			"/Users/me/My\\ File\\ \\(1\\)\\'s.txt /tmp/项目 ",
		);
		expect(formatDroppedPaths(["/tmp/a\nb's"], 'posix')).toBe("'/tmp/a\nb'\\''s' ");
	});
});
