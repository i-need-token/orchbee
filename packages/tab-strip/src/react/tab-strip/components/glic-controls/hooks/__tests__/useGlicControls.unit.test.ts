import { describe, expect, it } from 'vitest';

import { useGlicControls } from '../useGlicControls';

describe('useGlicControls', () => {
	it('provides the presenter used by the behavior-tested component adapter', () => {
		expect(useGlicControls).toBeTypeOf('function');
	});
});
