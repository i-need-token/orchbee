import { describe, expect, it } from 'vitest';

import { resolveWebRuntimeMode } from '../resolveWebRuntimeMode';

describe('resolveWebRuntimeMode', () => {
	it('starts nothing when neither the scheme nor User Agent identifies Electron', () => {
		expect(
			resolveWebRuntimeMode({
				protocol: 'https:',
				search: '',
				userAgent: 'Mozilla/5.0 Chrome/140.0.0.0 Safari/537.36',
			}),
		).toBe('unsupported');
		expect(
			resolveWebRuntimeMode({
				protocol: 'http:',
				search: '?unrelated=value',
				userAgent: 'Mozilla/5.0',
			}),
		).toBe('unsupported');
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

	it('rejects retired Gateway and unsupported explicit runtime markers', () => {
		const evidence = { protocol: 'http:', userAgent: 'Mozilla/5.0' } as const;
		expect(resolveWebRuntimeMode({ ...evidence, search: '?orchbee-runtime=gateway' })).toBe(
			'unsupported',
		);
		expect(resolveWebRuntimeMode({ ...evidence, search: '?orchbee-runtime=desktop' })).toBe(
			'unsupported',
		);
		expect(
			resolveWebRuntimeMode({
				...evidence,
				search: '?orchbee-runtime=gateway&orchbee-runtime=gateway',
			}),
		).toBe('unsupported');
		expect(resolveWebRuntimeMode({ ...evidence, search: '?orchbee-runtime=' })).toBe('unsupported');
	});
});
