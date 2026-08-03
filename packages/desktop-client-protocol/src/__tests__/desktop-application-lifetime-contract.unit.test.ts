import { describe, expect, it } from 'vitest';

import {
	DesktopApplicationLifetimeMainEventSchema,
	DesktopApplicationLifetimeRendererCommandSchema,
} from '../desktop-application-lifetime-contract';

describe('desktop application lifetime contract', () => {
	it('accepts only aggregate renderer preparation data', () => {
		expect(
			DesktopApplicationLifetimeRendererCommandSchema.parse({
				mode: 'stop',
				presentationCount: 3,
				requestId: 'exit-1',
				documentRevision: '4',
				type: 'desktop-application-exit-renderer-prepared',
			}),
		).toMatchObject({ mode: 'stop', presentationCount: 3 });
		expect(
			DesktopApplicationLifetimeRendererCommandSchema.safeParse({
				bindingIds: ['binding-1'],
				mode: 'stop',
				presentationCount: 3,
				requestId: 'exit-1',
				documentRevision: '4',
				type: 'desktop-application-exit-renderer-prepared',
			}).success,
		).toBe(false);
	});

	it('keeps Main failures typed and aggregate', () => {
		expect(
			DesktopApplicationLifetimeMainEventSchema.parse({
				requestId: 'exit-1',
				result: {
					failures: [{ code: 'host-orphan-detected', count: 2, retryable: false }],
					kind: 'failed',
				},
				type: 'desktop-application-exit-main-completed',
			}),
		).toMatchObject({ result: { kind: 'failed' } });
	});
});
