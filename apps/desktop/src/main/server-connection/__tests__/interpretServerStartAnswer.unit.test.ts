import { describe, expect, it } from 'vitest';

import { interpretServerStartAnswer } from '../interpretServerStartAnswer';

function restoreOf(stdout: string, failed: boolean, platform: NodeJS.Platform = 'linux') {
	return interpretServerStartAnswer(stdout, failed, platform).restore;
}

/** A failure envelope as `server start --format json` prints it. */
function failure(kind: string, details: Record<string, unknown>): string {
	return JSON.stringify({ failure: { details, kind }, schema: 'orchbee.cli.failure', version: 1 });
}

function admission(reason: string, versions: Record<string, string> = {}): string {
	return failure('incompatible', { reason, stage: 'admission', ...versions });
}

/** A success envelope as `server start --format json` prints it. */
function started(restoreNotice: string | null, extra: Record<string, unknown> = {}): string {
	return JSON.stringify({
		result: { outcome: 'started', restoreNotice, ...extra },
		schema: 'orchbee.server.start',
	});
}

describe('interpretServerStartAnswer', () => {
	it('gives each handshake refusal its own reason, naming both versions of a version refusal', () => {
		const versions = { clientVersion: '1.2.0', sessionVersion: '1.6.0' };
		expect(restoreOf(admission('client-too-old', versions), true)).toEqual({
			kind: 'refused',
			refusal: { desktopVersion: '1.2.0', reason: 'desktop-too-old', sessionVersion: '1.6.0' },
		});
		expect(restoreOf(admission('session-too-old', versions), true)).toEqual({
			kind: 'refused',
			refusal: {
				desktopVersion: '1.2.0',
				reason: 'session-too-old',
				sessionVersion: '1.6.0',
			},
		});
		expect(restoreOf(admission('environment-mismatch'), true)).toEqual({
			kind: 'refused',
			refusal: { reason: 'other-environment' },
		});
		expect(restoreOf(admission('fingerprint-mismatch'), true)).toEqual({
			kind: 'refused',
			refusal: { reason: 'development-build-mismatch' },
		});
	});

	it('leaves out the version of a Session that predates the handshake', () => {
		expect(restoreOf(admission('session-too-old', { clientVersion: '1.2.0' }), true)).toEqual({
			kind: 'refused',
			refusal: {
				desktopVersion: '1.2.0',
				reason: 'session-too-old',
				sessionVersion: null,
			},
		});
	});

	it('names a start refused until the Session is recovered', () => {
		const answer = failure('rejected', { reason: 'recovery-required', stage: 'activation' });
		expect(restoreOf(answer, true)).toEqual({
			kind: 'refused',
			refusal: { reason: 'recovery-required' },
		});
	});

	it('names the process that blocks recovery, and keeps the refusal when that name cannot be read', () => {
		const blockingProcess = {
			processId: 4242,
			program: 'orchbee-terminal-host.exe',
			role: 'terminal-host',
		};
		const blocked = failure('rejected', {
			blockingProcess,
			reason: 'recovery-required',
			stage: 'activation',
		});
		expect(restoreOf(blocked, true)).toEqual({
			kind: 'refused',
			refusal: { blockingProcess, reason: 'recovery-required' },
		});
		for (const unreadable of [
			{ ...blockingProcess, processId: 0 },
			{ ...blockingProcess, program: 'C:\\Orchbee\\orchbee-terminal-host.exe' },
			{ ...blockingProcess, role: 'agent' },
		]) {
			const answer = failure('rejected', {
				blockingProcess: unreadable,
				reason: 'recovery-required',
				stage: 'activation',
			});
			expect(restoreOf(answer, true)).toEqual({
				kind: 'refused',
				refusal: { reason: 'recovery-required' },
			});
		}
	});

	it('keeps what the system answered when it would not create the Server', () => {
		const answer = failure('rejected', {
			ioKind: 'permission-denied',
			osError: 5,
			reason: 'server-not-started',
			stage: 'activation',
		});
		expect(restoreOf(answer, true)).toEqual({
			kind: 'refused',
			refusal: { ioKind: 'permission-denied', osError: 5, reason: 'server-not-started' },
		});
	});

	it('names a refused folder with the rule it breaks under the system that refused it', () => {
		const path = 'C:\\Shared\\Orchbee';
		const answer = failure('check-failed', {
			refusal: { path, reason: 'others-can-read' },
			stage: 'runtime-base',
		});
		expect(restoreOf(answer, true, 'win32')).toEqual({
			kind: 'refused',
			refusal: { cause: 'others-can-read', path, reason: 'path-refused', system: 'windows' },
		});
		expect(restoreOf(answer, true, 'darwin')).toEqual({
			kind: 'refused',
			refusal: { cause: 'others-can-read', path, reason: 'path-refused', system: 'posix' },
		});
	});

	it('reads any failure it cannot name, or cannot read, as a restore that may have lost everything', () => {
		for (const answer of [
			// A reason a newer CLI names keeps the window's general sentence.
			failure('rejected', { reason: 'not-yet-named', stage: 'activation' }),
			failure('service-unavailable', { stage: 'admission' }),
			failure('incompatible', { stage: 'cohort' }),
			failure('check-failed', { stage: 'runtime-base' }),
			failure('check-failed', { refusal: { path: '/srv', reason: 'not-yet-named' } }),
			admission('initiator-too-new'),
			admission('client-too-old', { sessionVersion: '1.6.0' }),
			admission('client-too-old', { clientVersion: 'not a version' }),
			'',
			'{"failure":',
		]) {
			expect(restoreOf(answer, true)).toEqual({ kind: 'lost-or-unknown' });
		}
	});

	it('trusts a restore only when the activating process reports nothing lost', () => {
		expect(restoreOf(started(null), false)).toEqual({ kind: 'intact' });
		expect(restoreOf(started('recovered'), false)).toEqual({ kind: 'intact' });
		expect(restoreOf(started('empty'), false)).toEqual({
			kind: 'lost-or-unknown',
		});
		expect(restoreOf('not json', false)).toEqual({ kind: 'lost-or-unknown' });
	});

	it('names the negotiated capabilities only of a start that succeeded and lists them', () => {
		const names = ['agent-input-area', 'compatibility-window'];
		expect(
			interpretServerStartAnswer(started(null, { negotiatedCapabilities: names }), false, 'linux')
				.negotiatedCapabilities,
		).toEqual(names);
		expect(
			interpretServerStartAnswer(started(null), false, 'linux').negotiatedCapabilities,
		).toEqual([]);
		expect(
			interpretServerStartAnswer(
				admission('session-too-old', { clientVersion: '1.2.0' }),
				true,
				'linux',
			).negotiatedCapabilities,
		).toEqual([]);
		// A list outside the handshake's name grammar is no answer the window can trust.
		expect(
			interpretServerStartAnswer(
				started(null, { negotiatedCapabilities: ['Not A Name'] }),
				false,
				'linux',
			),
		).toEqual({
			negotiatedCapabilities: [],
			restore: { kind: 'lost-or-unknown' },
			sessionVersion: null,
		});
	});
});
