import { describe, expect, it } from 'vitest';

import { startsDesktopRendererDocument } from '../startsDesktopRendererDocument';

describe('startsDesktopRendererDocument', () => {
	it.each([
		{ expected: true, isMainFrame: true, isSameDocument: false },
		{ expected: false, isMainFrame: true, isSameDocument: true },
		{ expected: false, isMainFrame: false, isSameDocument: false },
		{ expected: false, isMainFrame: false, isSameDocument: true },
	] as const)(
		'returns $expected for main-frame=$isMainFrame same-document=$isSameDocument',
		({ expected, isMainFrame, isSameDocument }) => {
			expect(startsDesktopRendererDocument({ isMainFrame, isSameDocument })).toBe(expected);
		},
	);
});
