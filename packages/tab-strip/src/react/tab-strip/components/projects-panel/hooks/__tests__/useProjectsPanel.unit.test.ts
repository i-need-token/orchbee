import { describe, expect, it } from 'vitest';

import { useProjectsPanel } from '../useProjectsPanel';

describe('useProjectsPanel', () => {
	it('provides the presenter used by the behavior-tested component adapter', () => {
		expect(useProjectsPanel).toBeTypeOf('function');
	});
});
