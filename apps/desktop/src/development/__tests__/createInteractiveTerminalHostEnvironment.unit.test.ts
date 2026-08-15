import { describe, expect, it } from 'vitest';

import { createInteractiveTerminalHostEnvironment } from '../createInteractiveTerminalHostEnvironment';

describe('createInteractiveTerminalHostEnvironment', () => {
	it('removes inherited pager overrides without mutating the source environment', () => {
		const source = {
			GIT_PAGER: 'cat',
			LESS: '-R',
			PAGER: 'cat',
			PATH: '/usr/bin:/bin',
		};

		expect(createInteractiveTerminalHostEnvironment(source)).toEqual({
			LESS: '-R',
			PATH: '/usr/bin:/bin',
		});
		expect(source).toEqual({
			GIT_PAGER: 'cat',
			LESS: '-R',
			PAGER: 'cat',
			PATH: '/usr/bin:/bin',
		});
	});
});
