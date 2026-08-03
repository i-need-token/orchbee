import { describe, expectTypeOf, it } from 'vitest';

import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';

describe('ProjectedTerminalPlatformEffects', () => {
	it('defines Browser-owned clipboard and external-link effects', () => {
		expectTypeOf<ProjectedTerminalPlatformEffects>().toHaveProperty('openExternal');
		expectTypeOf<ProjectedTerminalPlatformEffects>().toHaveProperty('writeClipboardText');
	});
});
