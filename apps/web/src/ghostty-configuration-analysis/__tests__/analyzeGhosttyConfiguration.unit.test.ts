import { describe, expect, it } from 'vitest';

import { analyzeGhosttyConfiguration } from '../analyzeGhosttyConfiguration';

describe('analyzeGhosttyConfiguration', () => {
	it('parses the bounded Ghostty line syntax and reports every assignment in source order', () => {
		const analysis = analyzeGhosttyConfiguration(
			'# comment\r\n font-size = "14" \r\nforeground=#AbC\ncommand = echo=a\nbroken\n = value',
		);

		expect(analysis).toEqual({
			counts: { ignored: 1, invalid: 2, manual: 0, supported: 2 },
			findings: [
				{
					disposition: 'supported',
					line: 2,
					reason: 'lossless-target',
					source: 'font-size = "14"',
					target: { setting: 'font-size', value: 14 },
				},
				{
					disposition: 'supported',
					line: 3,
					reason: 'lossless-target',
					source: 'foreground=#AbC',
					target: { setting: 'custom-foreground', value: '#aabbcc' },
				},
				{
					disposition: 'ignored',
					line: 4,
					reason: 'not-in-whitelist',
					source: 'command = echo=a',
				},
				{
					disposition: 'invalid',
					line: 5,
					reason: 'malformed-line',
					source: 'broken',
				},
				{
					disposition: 'invalid',
					line: 6,
					reason: 'malformed-line',
					source: '= value',
				},
			],
			kind: 'report',
		});
	});

	it('projects only exactly representable current Orchbee settings', () => {
		const analysis = analyzeGhosttyConfiguration(
			[
				'font-family = JetBrains Mono',
				'font-size = 48',
				'foreground = FFF',
				'background = #123456',
				'background-opacity = 0.1',
			].join('\n'),
		);

		expect(analysis.kind).toBe('report');
		if (analysis.kind !== 'report') return;
		expect(analysis.counts).toEqual({ ignored: 0, invalid: 0, manual: 0, supported: 5 });
		expect(
			analysis.findings.map((finding) =>
				finding.disposition === 'supported' ? finding.target : null,
			),
		).toEqual([
			{ setting: 'font-family', value: 'JetBrains Mono' },
			{ setting: 'font-size', value: 48 },
			{ setting: 'custom-foreground', value: '#ffffff' },
			{ setting: 'custom-background', value: '#123456' },
			{ setting: 'background-opacity', value: 10 },
		]);
	});

	it('does not treat non-ASCII spacing as Ghostty configuration whitespace', () => {
		const analysis = analyzeGhosttyConfiguration('\u00a0font-size = 14');

		expect(analysis).toMatchObject({
			counts: { ignored: 1, invalid: 0, manual: 0, supported: 0 },
			findings: [{ disposition: 'ignored', line: 1, reason: 'not-in-whitelist' }],
		});
	});

	it('keeps invalid scalar assignments from superseding the last accepted value', () => {
		const analysis = analyzeGhosttyConfiguration(
			'font-size = 12\nfont-size = nope\nfont-size = 14\nfont-size = still-nope',
		);

		expect(analysis).toMatchObject({
			counts: { ignored: 1, invalid: 2, manual: 0, supported: 1 },
			findings: [
				{ disposition: 'ignored', line: 1, reason: 'superseded' },
				{ disposition: 'invalid', line: 2, reason: 'invalid-value' },
				{
					disposition: 'supported',
					line: 3,
					target: { setting: 'font-size', value: 14 },
				},
				{ disposition: 'invalid', line: 4, reason: 'invalid-value' },
			],
			kind: 'report',
		});
	});

	it('models font resets and refuses to collapse an active fallback list', () => {
		const fallback = analyzeGhosttyConfiguration('font-family = First\nfont-family = Second');
		const reset = analyzeGhosttyConfiguration(
			'font-family = Old\nfont-family = ""\nfont-family = Current',
		);

		expect(fallback).toMatchObject({
			counts: { ignored: 0, invalid: 0, manual: 2, supported: 0 },
			findings: [
				{ disposition: 'manual', line: 1, reason: 'font-family-fallback-list' },
				{ disposition: 'manual', line: 2, reason: 'font-family-fallback-list' },
			],
		});
		expect(reset).toMatchObject({
			counts: { ignored: 2, invalid: 0, manual: 0, supported: 1 },
			findings: [
				{ disposition: 'ignored', line: 1, reason: 'superseded' },
				{ disposition: 'ignored', line: 2, reason: 'reset-to-default' },
				{
					disposition: 'supported',
					line: 3,
					target: { setting: 'font-family', value: 'Current' },
				},
			],
		});
	});

	it('requires manual review for lossy values and cross-key cell opacity semantics', () => {
		const analysis = analyzeGhosttyConfiguration(
			[
				'font-size = 13.5',
				'foreground = rebeccapurple',
				'background-opacity = 0.85',
				'background-opacity-cells = true',
				'theme = light:Day,dark:Night',
				'window-theme = system',
				'keybind = ctrl+n=new_window',
				'working-directory = /tmp',
				'config-file = child.conf',
				'font-feature = -calt',
			].join('\n'),
		);

		expect(analysis).toMatchObject({
			counts: { ignored: 0, invalid: 0, manual: 10, supported: 0 },
			findings: [
				{ line: 1, reason: 'font-size-not-representable' },
				{ line: 2, reason: 'color-requires-review' },
				{ line: 3, reason: 'cell-opacity-not-supported' },
				{ line: 4, reason: 'cell-opacity-not-supported' },
				{ line: 5, reason: 'theme-requires-review' },
				{ line: 6, reason: 'window-theme-requires-review' },
				{ line: 7, reason: 'keybind-requires-review' },
				{ line: 8, reason: 'working-directory-requires-review' },
				{ line: 9, reason: 'config-file-not-loaded' },
				{ line: 10, reason: 'font-details-require-review' },
			],
		});
	});

	it('marks resets and non-whitelisted keys explicitly instead of dropping them', () => {
		const analysis = analyzeGhosttyConfiguration(
			'background =\nbackground-opacity-cells = false\ncursor-style = block',
		);

		expect(analysis).toMatchObject({
			counts: { ignored: 3, invalid: 0, manual: 0, supported: 0 },
			findings: [
				{ line: 1, reason: 'reset-to-default' },
				{ line: 2, reason: 'not-in-whitelist' },
				{ line: 3, reason: 'not-in-whitelist' },
			],
		});
	});

	it('rejects oversized sources instead of returning a partial report', () => {
		expect(analyzeGhosttyConfiguration('x'.repeat(262_145))).toEqual({
			kind: 'rejected',
			limit: 262_144,
			line: null,
			reason: 'source-too-large',
		});
		expect(analyzeGhosttyConfiguration('\n'.repeat(4096))).toEqual({
			kind: 'rejected',
			limit: 4096,
			line: null,
			reason: 'too-many-lines',
		});
		expect(analyzeGhosttyConfiguration(`#${'x'.repeat(4096)}`)).toEqual({
			kind: 'rejected',
			limit: 4096,
			line: 1,
			reason: 'line-too-long',
		});
		expect(analyzeGhosttyConfiguration(`#${'x'.repeat(4095)}`)).toEqual({
			counts: { ignored: 0, invalid: 0, manual: 0, supported: 0 },
			findings: [],
			kind: 'report',
		});
	});

	it('returns immutable deterministic reports without touching external state', () => {
		const first = analyzeGhosttyConfiguration('font-size = 8');
		const second = analyzeGhosttyConfiguration('font-size = 8');

		expect(first).toEqual(second);
		expect(Object.isFrozen(first)).toBe(true);
		if (first.kind !== 'report') return;
		expect(Object.isFrozen(first.findings)).toBe(true);
		expect(Object.isFrozen(first.counts)).toBe(true);
		expect(Object.isFrozen(first.findings[0])).toBe(true);
	});
});
