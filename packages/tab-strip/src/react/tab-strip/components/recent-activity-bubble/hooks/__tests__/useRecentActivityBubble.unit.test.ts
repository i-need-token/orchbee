import { describe, expect, it } from 'vitest';

import { useRecentActivityBubble } from '../useRecentActivityBubble';

describe('useRecentActivityBubble', () => {
	it('provides the presenter used by the behavior-tested component adapter', () => {
		expect(useRecentActivityBubble).toBeTypeOf('function');
	});
});
