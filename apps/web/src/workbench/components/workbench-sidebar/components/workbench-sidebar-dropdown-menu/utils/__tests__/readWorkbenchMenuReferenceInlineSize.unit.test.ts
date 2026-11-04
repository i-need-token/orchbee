// @vitest-environment jsdom

import { afterEach, describe, expect, it } from 'vitest';

import { readWorkbenchMenuReferenceInlineSize } from '../readWorkbenchMenuReferenceInlineSize';

afterEach(() => {
	document.documentElement.style.removeProperty('--ob-ref-size-menu');
	document.documentElement.style.removeProperty('font-size');
});

describe('readWorkbenchMenuReferenceInlineSize', () => {
	it('resolves the rem reference token against the root font size', () => {
		document.documentElement.style.setProperty('--ob-ref-size-menu', '15.625rem');
		document.documentElement.style.fontSize = '16px';

		expect(readWorkbenchMenuReferenceInlineSize(document)).toBe(250);
	});

	it('reads a pixel reference token directly', () => {
		document.documentElement.style.setProperty('--ob-ref-size-menu', '240px');

		expect(readWorkbenchMenuReferenceInlineSize(document)).toBe(240);
	});

	it.each(['', '0px', '50%'])('returns null for an unavailable token value: %s', (tokenValue) => {
		document.documentElement.style.setProperty('--ob-ref-size-menu', tokenValue);

		expect(readWorkbenchMenuReferenceInlineSize(document)).toBeNull();
	});
});
