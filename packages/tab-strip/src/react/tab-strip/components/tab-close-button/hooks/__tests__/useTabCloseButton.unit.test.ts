import { describe, expect, it } from 'vitest';

import { useTabCloseButton } from '../useTabCloseButton';

describe('useTabCloseButton', () => {
	it('provides the presenter used by the behavior-tested component adapter', () => {
		expect(useTabCloseButton).toBeTypeOf('function');
	});
});
