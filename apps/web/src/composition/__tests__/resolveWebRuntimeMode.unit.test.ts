import { describe, expect, it } from 'vitest';

import { resolveWebRuntimeMode } from '../resolveWebRuntimeMode';

describe('resolveWebRuntimeMode', () => {
	it('uses Browser mode when neither the scheme nor User Agent identifies Electron', () => {
		expect(
			resolveWebRuntimeMode({
				protocol: 'https:',
				search: '',
				userAgent: 'Mozilla/5.0 Chrome/140.0.0.0 Safari/537.36',
			}),
		).toBe('browser');
		expect(
			resolveWebRuntimeMode({
				protocol: 'http:',
				search: '?unrelated=value',
				userAgent: 'Mozilla/5.0',
			}),
		).toBe('browser');
	});

	it('selects Desktop mode from the packaged scheme or the Electron User Agent', () => {
		expect(
			resolveWebRuntimeMode({ protocol: 'orchbee:', search: '', userAgent: 'Mozilla/5.0' }),
		).toBe('desktop');
		expect(
			resolveWebRuntimeMode({
				protocol: 'http:',
				search: '',
				userAgent: 'Mozilla/5.0 AppleWebKit/537.36 Chrome/140.0.0.0 Electron/42.8.0 Safari/537.36',
			}),
		).toBe('desktop');
	});

	it('requires one exact explicit marker only for a Gateway client', () => {
		const evidence = { protocol: 'http:', userAgent: 'Mozilla/5.0' } as const;
		expect(resolveWebRuntimeMode({ ...evidence, search: '?orchbee-runtime=gateway' })).toBe(
			'gateway',
		);
		expect(resolveWebRuntimeMode({ ...evidence, search: '?orchbee-runtime=desktop' })).toBe(
			'invalid',
		);
		expect(
			resolveWebRuntimeMode({
				...evidence,
				search: '?orchbee-runtime=gateway&orchbee-runtime=gateway',
			}),
		).toBe('invalid');
		expect(resolveWebRuntimeMode({ ...evidence, search: '?orchbee-runtime=' })).toBe('invalid');
	});
});
