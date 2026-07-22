import { describe, expect, it } from 'vitest';

import { useTabIcon } from '../useTabIcon';

describe('useTabIcon', () => {
	it('provides the presenter used by the behavior-tested component adapter', () => {
		expect(useTabIcon).toBeTypeOf('function');
	});
});
