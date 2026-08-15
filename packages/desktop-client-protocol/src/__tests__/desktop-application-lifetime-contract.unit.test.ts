import { describe, expect, it } from 'vitest';

import {
	DesktopApplicationLifetimeMainEventSchema,
	DesktopApplicationLifetimeRendererCommandSchema,
} from '../desktop-application-lifetime-contract';

describe('desktop application lifetime contract', () => {
	it('keeps renderer preparation data aggregate', () => {
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

	it('admits the development Host restart as an explicit automatic lifetime mode', () => {
		expect(
			DesktopApplicationLifetimeMainEventSchema.parse({
				reason: 'development-host-restart',
				restartPresentationIds: ['presentation-1'],
				requestId: 'exit-development-host',
				type: 'desktop-application-exit-requested',
			}),
		).toMatchObject({
			reason: 'development-host-restart',
			restartPresentationIds: ['presentation-1'],
		});
		expect(
			DesktopApplicationLifetimeRendererCommandSchema.parse({
				choice: 'development-host-restart',
				restartPresentationIds: ['presentation-1'],
				requestId: 'exit-development-host',
				type: 'desktop-application-exit-choice',
			}),
		).toMatchObject({
			choice: 'development-host-restart',
			restartPresentationIds: ['presentation-1'],
		});
		expect(
			DesktopApplicationLifetimeRendererCommandSchema.safeParse({
				choice: 'development-host-restart',
				requestId: 'exit-development-host',
				type: 'desktop-application-exit-choice',
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
