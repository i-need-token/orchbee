import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION } from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE } from '../terminal-presentation-configuration-source';

describe('default terminal presentation configuration source', () => {
	it('provisions the Ghostty default faces and never emits runtime updates', () => {
		const listener = vi.fn<() => void>();
		const dispose = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.subscribe(listener);
		const current = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.current();

		expect(current).toEqual({
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			fontFamily: '"Orchbee JetBrains Mono", "Orchbee Symbols Nerd Font", monospace',
			fontSize: 13,
		});
		expect(DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.current()).toBe(current);
		expect(listener).not.toHaveBeenCalled();
		expect(dispose()).toBeUndefined();
	});
});
