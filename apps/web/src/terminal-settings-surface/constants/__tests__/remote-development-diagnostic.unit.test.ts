import { execFileSync } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import {
	REMOTE_DEVELOPMENT_BASELINE_TERM,
	REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND,
} from '../remote-development-diagnostic';

describe('remote development diagnostic', () => {
	it('reports the bounded environment and distinct terminfo outcomes without a mutation tool', () => {
		expect(REMOTE_DEVELOPMENT_BASELINE_TERM).toBe('xterm-256color');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('ORCHBEE_REMOTE_DIAGNOSTIC_V1');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('orchbee_term="${TERM-}"');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain(
			'orchbee_report_env TERM "$orchbee_term"',
		);
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain(
			'orchbee_report_env COLORTERM "${COLORTERM-}"',
		);
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain(
			'orchbee_report_env TERM_PROGRAM "${TERM_PROGRAM-}"',
		);
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('TERM_MATCH=expected');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('TERM_MATCH=different');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('TERMINFO=term-unset');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('TERMINFO=unsafe-term');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('TERMINFO=infocmp-unavailable');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('TERMINFO=available');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('TERMINFO=missing');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('command -v infocmp');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('command infocmp');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('unsafe-value');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toContain('*[!A-Za-z0-9._+-]*');
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).toMatch(/^\( .* \)$/u);
		expect(REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND).not.toMatch(
			/(?:^|[;&|\s])(curl|mkdir|scp|ssh|sudo|tic|wget)(?:$|[;&|\s])/u,
		);
	});

	it.skipIf(process.platform === 'win32')(
		'does not print control-bearing or oversized values and leaves no shell function behind',
		() => {
			const output = execFileSync(
				'/bin/sh',
				[
					'-c',
					`${REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND}; if command -v orchbee_report_env >/dev/null 2>&1; then printf 'FUNCTION_LEAK=yes\\n'; else printf 'FUNCTION_LEAK=no\\n'; fi`,
				],
				{
					encoding: 'utf8',
					env: {
						COLORTERM: 'truecolor\nunsafe',
						PATH: '',
						TERM: `xterm-\u001B]0;unsafe`,
						TERM_PROGRAM: 'x'.repeat(129),
					},
				},
			);

			expect(output).toContain('TERM=unsafe-value');
			expect(output).toContain('COLORTERM=unsafe-value');
			expect(output).toContain('TERM_PROGRAM=unsafe-value');
			expect(output).toContain('TERMINFO=unsafe-term');
			expect(output).toContain('FUNCTION_LEAK=no');
			expect(output).not.toContain('\u001B');
		},
	);
});
