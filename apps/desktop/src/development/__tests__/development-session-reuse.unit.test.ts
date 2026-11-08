import { describe, expect, it } from 'vitest';

import { keepDevelopmentSession, keepsDevelopmentSession } from '../development-session-reuse';

const digest = (digit: string): string => digit.repeat(64);

/** A staged `cohort.json`, as the staging script writes it. */
function manifest(overrides: Record<string, string> = {}) {
	return {
		artifacts: {
			orchbee: digest('1'),
			'orchbee-server': digest('2'),
			'orchbee-worker': digest('3'),
			'orchbee-terminal-host': digest('4'),
			'agent-observation-adapters': digest('5'),
			...overrides,
		},
		schemaVersion: 1,
	};
}

describe('keepsDevelopmentSession', () => {
	it('keeps the Session when only the Client or the Capsule changed', () => {
		expect(keepsDevelopmentSession(manifest(), manifest())).toBe(true);
		expect(
			keepsDevelopmentSession(
				manifest(),
				manifest({ orchbee: digest('a'), 'agent-observation-adapters': digest('b') }),
			),
		).toBe(true);
	});

	it('stops the Session when its Server, Worker, or Terminal Host changed', () => {
		for (const role of ['orchbee-server', 'orchbee-worker', 'orchbee-terminal-host']) {
			expect(keepsDevelopmentSession(manifest(), manifest({ [role]: digest('f') }))).toBe(false);
		}
	});

	it('stops the Session when either manifest cannot be read', () => {
		const unreadable: unknown[] = [
			undefined,
			null,
			{ ...manifest(), schemaVersion: 2 },
			{
				artifacts: { 'orchbee-server': digest('2'), 'orchbee-worker': digest('3') },
				schemaVersion: 1,
			},
			manifest({ 'orchbee-server': 'not a digest' }),
		];
		for (const other of unreadable) {
			expect(keepsDevelopmentSession(other, manifest())).toBe(false);
			expect(keepsDevelopmentSession(manifest(), other)).toBe(false);
		}
	});
});

describe('keepDevelopmentSession', () => {
	const started = 'cohort-0000000000000001';
	const lastClient = 'cohort-0000000000000002';
	const current = 'cohort-0000000000000003';
	const marker = { client: lastClient, cohort: started, pid: 41 };

	it('keeps naming the starting cohort while its Server still runs', () => {
		expect(keepDevelopmentSession(marker, current, { kind: 'running', pid: 41 })).toEqual({
			client: current,
			cohort: started,
			pid: 41,
		});
	});

	it('names the last Client’s cohort for a Server started since', () => {
		expect(keepDevelopmentSession(marker, current, { kind: 'running', pid: 97 })).toEqual({
			client: current,
			cohort: lastClient,
			pid: 97,
		});
		expect(
			keepDevelopmentSession({ client: lastClient, cohort: started }, current, {
				kind: 'running',
				pid: 41,
			}).cohort,
		).toBe(lastClient);
	});

	it('names the current cohort for a stopped Session, which its Client will start', () => {
		expect(keepDevelopmentSession(marker, current, { kind: 'stopped' })).toEqual({
			client: current,
			cohort: current,
		});
	});

	it('keeps the starting cohort when nothing says whether the Session runs', () => {
		expect(keepDevelopmentSession(marker, current, { kind: 'unknown' })).toEqual({
			client: current,
			cohort: started,
			pid: 41,
		});
	});
});
