import { describe, expect, it } from 'vitest';
import { formatTerminalIdentity } from '../terminal-identity';

describe('formatTerminalIdentity', () => {
	it('uses the Orchbee package identity by default', () => {
		expect(formatTerminalIdentity()).toBe('orchbee 0.1.0');
	});

	it('removes control bytes and bounds the response', () => {
		const value = formatTerminalIdentity({ name: ' custom\x1bterm ', version: '2.0\n' });
		expect(value).toBe('customterm 2.0');
		expect(formatTerminalIdentity({ name: 'x'.repeat(300), version: '1' })).toHaveLength(256);
	});
});
