import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION } from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE } from '../terminal-presentation-configuration-source';

describe('default terminal presentation configuration source', () => {
	it('keeps one stable default and never emits runtime updates', () => {
		const listener = vi.fn<() => void>();
		const dispose = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.subscribe(listener);

		expect(DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.current()).toBe(
			DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
		);
		expect(listener).not.toHaveBeenCalled();
		expect(dispose()).toBeUndefined();
	});
});
