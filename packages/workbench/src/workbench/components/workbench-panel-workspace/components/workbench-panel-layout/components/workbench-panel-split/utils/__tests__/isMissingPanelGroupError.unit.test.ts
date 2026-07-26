import { describe, expect, it } from 'vitest';

import { isMissingPanelGroupError } from '../isMissingPanelGroupError';

describe('isMissingPanelGroupError', () => {
	it('recognizes a missing primitive group registration', () => {
		expect(isMissingPanelGroupError(new Error("Could not find Group with id 'split-1'"))).toBe(
			true,
		);
	});

	it('rejects unrelated values and errors', () => {
		expect(isMissingPanelGroupError(new Error('Panel ids must be unique'))).toBe(false);
		expect(isMissingPanelGroupError('Could not find Group with id split-1')).toBe(false);
	});
});
