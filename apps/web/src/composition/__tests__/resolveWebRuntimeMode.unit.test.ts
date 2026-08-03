import { describe, expect, it } from 'vitest';

import { resolveWebRuntimeMode } from '../resolveWebRuntimeMode';

describe('resolveWebRuntimeMode', () => {
	it('uses Browser mode when no runtime marker is present', () => {
		expect(resolveWebRuntimeMode('')).toBe('browser');
		expect(resolveWebRuntimeMode('?unrelated=value')).toBe('browser');
	});

	it('selects one exact explicit shared-client marker', () => {
		expect(resolveWebRuntimeMode('?orchbee-runtime=desktop')).toBe('desktop');
		expect(resolveWebRuntimeMode('?orchbee-runtime=gateway')).toBe('gateway');
		expect(resolveWebRuntimeMode('?orchbee-runtime=browser')).toBe('invalid');
		expect(resolveWebRuntimeMode('?orchbee-runtime=desktop&orchbee-runtime=desktop')).toBe(
			'invalid',
		);
		expect(resolveWebRuntimeMode('?orchbee-runtime=')).toBe('invalid');
	});
});
