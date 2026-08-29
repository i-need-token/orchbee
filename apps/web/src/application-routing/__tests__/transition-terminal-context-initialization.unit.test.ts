import { describe, expect, it } from 'vitest';

import {
	EMPTY_TERMINAL_CONTEXT_INITIALIZATION_STATE,
	transitionTerminalContextInitialization,
} from '../transition-terminal-context-initialization';

describe('transitionTerminalContextInitialization', () => {
	it('does not decide a context from redirects or unavailable routes', () => {
		const initial = EMPTY_TERMINAL_CONTEXT_INITIALIZATION_STATE;

		expect(transitionTerminalContextInitialization(initial, { type: 'redirect-observed' })).toEqual(
			{ commands: [], state: [] },
		);
		expect(transitionTerminalContextInitialization(initial, { type: 'route-unavailable' })).toEqual(
			{ commands: [], state: [] },
		);
	});

	it('creates exactly when a context is empty on its first ready visit', () => {
		expect(
			transitionTerminalContextInitialization([], {
				hasTabs: false,
				terminalContextId: 'context-1',
				type: 'terminal-route-ready',
			}),
		).toEqual({
			commands: [{ terminalContextId: 'context-1', type: 'create-terminal' }],
			state: ['context-1'],
		});
		expect(
			transitionTerminalContextInitialization([], {
				hasTabs: true,
				terminalContextId: 'context-1',
				type: 'terminal-route-ready',
			}),
		).toEqual({ commands: [], state: ['context-1'] });
	});

	it('decides each context independently and never repeats a decided context', () => {
		const first = transitionTerminalContextInitialization([], {
			hasTabs: true,
			terminalContextId: 'standalone',
			type: 'terminal-route-ready',
		});
		const repeated = transitionTerminalContextInitialization(first.state, {
			hasTabs: false,
			terminalContextId: 'standalone',
			type: 'terminal-route-ready',
		});
		const workspace = transitionTerminalContextInitialization(repeated.state, {
			hasTabs: false,
			terminalContextId: 'workspace',
			type: 'terminal-route-ready',
		});

		expect(repeated).toEqual({ commands: [], state: ['standalone'] });
		expect(workspace).toEqual({
			commands: [{ terminalContextId: 'workspace', type: 'create-terminal' }],
			state: ['standalone', 'workspace'],
		});
	});
});
