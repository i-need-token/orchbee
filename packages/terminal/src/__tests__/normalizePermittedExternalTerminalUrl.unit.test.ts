import { describe, expect, it } from 'vitest';

import { normalizePermittedExternalTerminalUrl } from '../normalizePermittedExternalTerminalUrl';

describe('external terminal navigation policy', () => {
	it.each([
		['http://orchbee.dev/path', 'http://orchbee.dev/path'],
		['HTTPS://orchbee.dev/终端', 'https://orchbee.dev/%E7%BB%88%E7%AB%AF'],
		['mailto:terminal@orchbee.dev', 'mailto:terminal@orchbee.dev'],
	] as const)('normalizes the approved target %s', (target, expected) => {
		expect(normalizePermittedExternalTerminalUrl(target)).toBe(expected);
	});

	it.each([
		'file:///tmp/secret',
		'ftp://orchbee.dev',
		'ssh://orchbee.dev',
		'tel:+15551234567',
		'data:text/plain,secret',
		'javascript:alert(1)',
		'/relative/path',
		'not a URL',
	] as const)('rejects the unapproved target %s', (target) => {
		expect(normalizePermittedExternalTerminalUrl(target)).toBeNull();
	});
});
