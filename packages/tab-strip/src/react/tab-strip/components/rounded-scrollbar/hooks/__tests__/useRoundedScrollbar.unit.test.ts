import { describe, expect, it } from 'vitest';

import { useRoundedScrollbar } from '../useRoundedScrollbar';

describe('useRoundedScrollbar', () => {
	it('provides the presenter used by the behavior-tested component adapter', () => {
		expect(useRoundedScrollbar).toBeTypeOf('function');
	});
});
