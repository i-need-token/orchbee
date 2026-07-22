import { describe, expect, it } from 'vitest';

import { useAlertIndicatorButton } from '../useAlertIndicatorButton';

describe('useAlertIndicatorButton', () => {
	it('provides the presenter used by the behavior-tested component adapter', () => {
		expect(useAlertIndicatorButton).toBeTypeOf('function');
	});
});
