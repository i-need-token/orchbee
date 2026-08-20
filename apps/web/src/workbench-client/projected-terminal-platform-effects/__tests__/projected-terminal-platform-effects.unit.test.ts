import { describe, expectTypeOf, it } from 'vitest';

import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';

describe('ProjectedTerminalPlatformEffects', () => {
	it('defines Browser-owned clipboard, link, and independent opt-in bell effects', () => {
		expectTypeOf<ProjectedTerminalPlatformEffects>().toHaveProperty('isClipboardWriteAvailable');
		expectTypeOf<ProjectedTerminalPlatformEffects>().toHaveProperty('openExternal');
		expectTypeOf<ProjectedTerminalPlatformEffects>().toHaveProperty('writeClipboardText');
		expectTypeOf<ProjectedTerminalPlatformEffects>().toHaveProperty('playBellSound');
		expectTypeOf<ProjectedTerminalPlatformEffects>().toHaveProperty('showBellNotification');
	});
});
