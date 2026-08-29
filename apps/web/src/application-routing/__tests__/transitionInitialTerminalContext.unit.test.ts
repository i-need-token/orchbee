import { describe, expect, it } from 'vitest';

import {
	transitionInitialTerminalContext,
	type InitialTerminalContextEvent,
} from '../transitionInitialTerminalContext';

describe('transitionInitialTerminalContext', () => {
	it('waits through startup redirects', () => {
		expect(
			transitionInitialTerminalContext('awaiting-route', { type: 'redirect-observed' }),
		).toEqual({ commands: [], state: 'awaiting-route' });
	});

	it('settles an unavailable initial route without creating a terminal', () => {
		expect(
			transitionInitialTerminalContext('awaiting-route', { type: 'route-unavailable' }),
		).toEqual({ commands: [], state: 'settled' });
	});

	it('creates exactly when the first ready terminal context is empty', () => {
		expect(
			transitionInitialTerminalContext('awaiting-route', {
				hasTabs: false,
				type: 'terminal-route-ready',
			}),
		).toEqual({ commands: [{ type: 'create-terminal' }], state: 'settled' });
		expect(
			transitionInitialTerminalContext('awaiting-route', {
				hasTabs: true,
				type: 'terminal-route-ready',
			}),
		).toEqual({ commands: [], state: 'settled' });
	});

	it('never creates after the startup decision has settled', () => {
		const events: readonly InitialTerminalContextEvent[] = [
			{ type: 'redirect-observed' },
			{ type: 'route-unavailable' },
			{ hasTabs: false, type: 'terminal-route-ready' },
			{ hasTabs: true, type: 'terminal-route-ready' },
		];

		for (const event of events) {
			expect(transitionInitialTerminalContext('settled', event)).toEqual({
				commands: [],
				state: 'settled',
			});
		}
	});
});
