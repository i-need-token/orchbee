import { describe, expect, it } from 'vitest';

import { interpretServerStartAnswer } from '../interpretServerStartAnswer';

/** A failure envelope as `server start --format json` prints it. */
function failure(kind: string, details: Record<string, string>): string {
	return JSON.stringify({ failure: { details, kind }, schema: 'orchbee.cli.failure', version: 1 });
}

function admission(reason: string, versions: Record<string, string> = {}): string {
	return failure('incompatible', { reason, stage: 'admission', ...versions });
}

/** A success envelope as `server start --format json` prints it. */
function started(restoreNotice: string | null): string {
	return JSON.stringify({
		result: { outcome: 'started', restoreNotice },
		schema: 'orchbee.server.start',
	});
}

describe('interpretServerStartAnswer', () => {
	it('gives each handshake refusal its own reason, naming both versions of a version refusal', () => {
		const versions = { clientVersion: '1.2.0', sessionVersion: '1.6.0' };
		expect(interpretServerStartAnswer(admission('client-too-old', versions), true)).toEqual({
			kind: 'refused',
			refusal: { desktopVersion: '1.2.0', reason: 'desktop-too-old', sessionVersion: '1.6.0' },
		});
		expect(interpretServerStartAnswer(admission('session-too-old', versions), true)).toEqual({
			kind: 'refused',
			refusal: { desktopVersion: '1.2.0', reason: 'session-too-old', sessionVersion: '1.6.0' },
		});
		expect(interpretServerStartAnswer(admission('environment-mismatch'), true)).toEqual({
			kind: 'refused',
			refusal: { reason: 'other-environment' },
		});
		expect(interpretServerStartAnswer(admission('fingerprint-mismatch'), true)).toEqual({
			kind: 'refused',
			refusal: { reason: 'development-build-mismatch' },
		});
	});

	it('leaves out the version of a Session that predates the handshake', () => {
		expect(
			interpretServerStartAnswer(admission('session-too-old', { clientVersion: '1.2.0' }), true),
		).toEqual({
			kind: 'refused',
			refusal: { desktopVersion: '1.2.0', reason: 'session-too-old', sessionVersion: null },
		});
	});

	it('names a start refused until the Session is recovered', () => {
		const answer = failure('rejected', { reason: 'recovery-required', stage: 'activation' });
		expect(interpretServerStartAnswer(answer, true)).toEqual({
			kind: 'refused',
			refusal: { reason: 'recovery-required' },
		});
	});

	it('reads any failure it cannot name, or cannot read, as a restore that may have lost everything', () => {
		for (const answer of [
			failure('service-unavailable', { stage: 'admission' }),
			failure('incompatible', { stage: 'cohort' }),
			admission('initiator-too-new'),
			admission('client-too-old', { sessionVersion: '1.6.0' }),
			admission('client-too-old', { clientVersion: 'not a version' }),
			'',
			'{"failure":',
		]) {
			expect(interpretServerStartAnswer(answer, true)).toEqual({ kind: 'lost-or-unknown' });
		}
	});

	it('trusts a restore only when the activating process reports nothing lost', () => {
		expect(interpretServerStartAnswer(started(null), false)).toEqual({ kind: 'intact' });
		expect(interpretServerStartAnswer(started('recovered'), false)).toEqual({ kind: 'intact' });
		expect(interpretServerStartAnswer(started('empty'), false)).toEqual({
			kind: 'lost-or-unknown',
		});
		expect(interpretServerStartAnswer('not json', false)).toEqual({ kind: 'lost-or-unknown' });
	});
});
