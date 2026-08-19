import { describe, expect, it } from 'vitest';

import { isTerminalThemePresentationSource } from '../isTerminalThemePresentationSource';

describe('isTerminalThemePresentationSource', () => {
	it('accepts only objects with a participant registration function', () => {
		expect(
			isTerminalThemePresentationSource({
				registerThemeParticipant: () => () => undefined,
			}),
		).toBe(true);
		expect(isTerminalThemePresentationSource({ registerThemeParticipant: true })).toBe(false);
		expect(isTerminalThemePresentationSource({})).toBe(false);
		expect(isTerminalThemePresentationSource(null)).toBe(false);
	});
});
