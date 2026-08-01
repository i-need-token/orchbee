import { describe, expect, it } from 'vitest';

import {
	readWorkbenchConfirmationAnchor,
	workbenchConfirmationCapabilities,
} from '../workbench-confirmation-invocation';

describe('workbench confirmation invocation', () => {
	it('round-trips a contextual anchor through Action capabilities', () => {
		const anchor = { x: 320, y: 240 };

		expect(readWorkbenchConfirmationAnchor(workbenchConfirmationCapabilities(anchor))).toEqual(
			anchor,
		);
	});

	it.each([
		undefined,
		{},
		{ 'workbench.confirmation-anchor': null },
		{ 'workbench.confirmation-anchor': { x: '320', y: 240 } },
		{ 'workbench.confirmation-anchor': { x: 320 } },
	])('rejects malformed contextual anchor capabilities', (capabilities) => {
		expect(readWorkbenchConfirmationAnchor(capabilities)).toBeNull();
	});
});
