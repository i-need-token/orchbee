import { describe, expect, it } from 'vitest';

import { ORCHBEE_LOCALES } from '../../locale';
import { createOrchbeeIntl } from '../createOrchbeeIntl';

describe('createOrchbeeIntl', () => {
	it('creates a formatter for every shipped locale', () => {
		for (const locale of ORCHBEE_LOCALES) {
			const intl = createOrchbeeIntl(locale);
			expect(intl.locale).toBe(locale);
			expect(intl.formatNumber(1234).length).toBeGreaterThan(0);
		}
	});
});
