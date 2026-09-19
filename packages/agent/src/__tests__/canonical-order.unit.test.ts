import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { compareAgentIdentity } from '../model';

describe('compareAgentIdentity', () => {
	it.each([
		['B', 'a'],
		['A', 'a'],
		['I', 'i'],
		['9', 'A'],
		[':', 'A'],
		['-', '.'],
		['.', '0'],
		['10', '2'],
		['_', 'a'],
		['e\u0301', '\u00e9'],
		['ab', 'a\u00adb'],
		['ab', 'a\u200bb'],
		['a\u0000b', 'ab'],
		['中', '文'],
		['\u{10000}', '\ue000'],
		['\u{1f600}', '\uffff'],
		['z', '\u{1f600}'],
	] as const)('orders %j before %j', (lower, higher) => {
		expect(compareAgentIdentity(lower, higher)).toBeLessThan(0);
		expect(compareAgentIdentity(higher, lower)).toBeGreaterThan(0);
	});

	it('compares supplementary code points by UTF-16 code units, not scalar values', () => {
		expect(compareAgentIdentity('\ue000', '\u{10000}')).toBeGreaterThan(0);
		expect(compareAgentIdentity('\uffff', '\u{1f600}')).toBeGreaterThan(0);
	});

	it('returns zero only for identical strings', () => {
		expect(compareAgentIdentity('a', 'a')).toBe(0);
		expect(compareAgentIdentity('\u00e9', 'e\u0301')).not.toBe(0);
	});

	it('is antisymmetric and consistent with equality on arbitrary strings', () => {
		fc.assert(
			fc.property(fc.string(), fc.string(), (a, b) => {
				expect(Math.sign(compareAgentIdentity(a, b)) + Math.sign(compareAgentIdentity(b, a))).toBe(
					0,
				);
				expect(compareAgentIdentity(a, b) === 0).toBe(a === b);
			}),
			{ numRuns: 500, seed: 0x0a6e17 },
		);
	});

	it('is transitive on arbitrary triples', () => {
		fc.assert(
			fc.property(fc.string(), fc.string(), fc.string(), (a, b, c) => {
				const antecedent = compareAgentIdentity(a, b) <= 0 && compareAgentIdentity(b, c) <= 0;
				expect(!antecedent || compareAgentIdentity(a, c) <= 0).toBe(true);
			}),
			{ numRuns: 500, seed: 0x0a6e17 },
		);
	});

	it('totally orders every permutation of a fixed set into one canonical order', () => {
		const corpus = [
			's:z',
			's:_',
			's:Z',
			's:9',
			's:.',
			's::',
			's:a',
			's:-',
			's:1',
			's:A',
			's:i',
			's:I',
			'\u00e9',
			'e\u0301',
			'a\u00adb',
			'ab',
			'a\u200bb',
			'中',
			'文',
			'\ue000',
			'\u{1f600}',
		];
		const canonical = [...corpus].sort(compareAgentIdentity);
		fc.assert(
			fc.property(
				fc.shuffledSubarray(corpus, { maxLength: corpus.length, minLength: corpus.length }),
				(permutation) => {
					expect([...permutation].sort(compareAgentIdentity)).toEqual(canonical);
				},
			),
			{ numRuns: 100, seed: 0x0a6e17 },
		);
		expect(canonical).toEqual([
			'ab',
			'a\u00adb',
			'a\u200bb',
			'e\u0301',
			's:-',
			's:.',
			's:1',
			's:9',
			's::',
			's:A',
			's:I',
			's:Z',
			's:_',
			's:a',
			's:i',
			's:z',
			'\u00e9',
			'\u4e2d',
			'\u6587',
			'\u{1f600}',
			'\ue000',
		]);
	});
});
