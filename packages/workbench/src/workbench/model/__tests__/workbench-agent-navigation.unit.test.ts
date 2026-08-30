import { AgentSessionIdSchema } from '@orchbee/agent';
import { describe, expect, it } from 'vitest';

import {
	INITIAL_WORKBENCH_AGENT_NAVIGATION_STATE,
	transitionWorkbenchAgentNavigation,
	type WorkbenchAgentNavigationEvent,
	type WorkbenchAgentNavigationState,
} from '../workbench-agent-navigation';

describe('transitionWorkbenchAgentNavigation', () => {
	it('focuses exactly once when action settlement follows the ready projection', () => {
		let state = mount();
		const navigation = transitionWorkbenchAgentNavigation(state, navigate(1, 'session-a'));
		state = navigation.state;
		expect(navigation.commands).toEqual([
			{
				input: input('session-a'),
				kind: 'invoke-agent-navigation',
				requestId: 1,
			},
		]);
		state = apply(state, projection('ready'));
		const settled = transitionWorkbenchAgentNavigation(state, {
			kind: 'action-settled',
			outcome: 'accepted',
			requestId: 1,
		});
		expect(settled).toEqual({
			commands: [{ kind: 'request-terminal-focus' }],
			state: { kind: 'idle' },
		});
		expect(transitionWorkbenchAgentNavigation(settled.state, projection('ready')).commands).toEqual(
			[],
		);
	});

	it('waits for the exact projection when action settlement arrives first', () => {
		let state = apply(mount(), navigate(1, 'session-a'));
		state = apply(state, { kind: 'action-settled', outcome: 'accepted', requestId: 1 });
		const ready = transitionWorkbenchAgentNavigation(state, projection('ready'));
		expect(ready.commands).toEqual([{ kind: 'request-terminal-focus' }]);
		expect(ready.state).toEqual({ kind: 'idle' });
	});

	it('lets the latest click replace an older request without late focus theft', () => {
		let state = apply(mount(), navigate(1, 'session-a'));
		state = apply(state, navigate(2, 'session-b'));
		state = apply(state, { kind: 'action-settled', outcome: 'accepted', requestId: 1 });
		expect(state).toMatchObject({ kind: 'waiting', requestId: 2 });
		state = apply(state, projection('ready'));
		const latest = transitionWorkbenchAgentNavigation(state, {
			kind: 'action-settled',
			outcome: 'accepted',
			requestId: 2,
		});
		expect(latest.commands).toEqual([{ kind: 'request-terminal-focus' }]);
	});

	it('announces invalidation and cancels navigation when the Session disappears', () => {
		const state = apply(mount(), navigate(1, 'session-a'));
		expect(transitionWorkbenchAgentNavigation(state, projection('unavailable'))).toEqual({
			commands: [{ kind: 'announce-unavailable' }],
			state: { kind: 'idle' },
		});
	});

	it('cancels when unrelated navigation overtakes the pending route', () => {
		const state = apply(mount(), navigate(1, 'session-a'));
		const transition = transitionWorkbenchAgentNavigation(state, {
			activeLocation: {
				kind: 'workspace',
				projectId: 'project-b',
				workspaceId: 'workspace-b',
			},
			kind: 'projection-observed',
			projection: 'pending',
		});
		expect(transition).toEqual({ commands: [], state: { kind: 'idle' } });
	});

	it('ignores late settlement and projection events after disposal', () => {
		const disposed = transitionWorkbenchAgentNavigation(apply(mount(), navigate(1, 'session-a')), {
			kind: 'dispose',
		});
		expect(disposed).toEqual({ commands: [], state: { kind: 'disposed' } });
		expect(
			transitionWorkbenchAgentNavigation(disposed.state, {
				kind: 'action-settled',
				outcome: 'accepted',
				requestId: 1,
			}),
		).toEqual({ commands: [], state: { kind: 'disposed' } });
		expect(transitionWorkbenchAgentNavigation(disposed.state, projection('ready'))).toEqual({
			commands: [],
			state: { kind: 'disposed' },
		});
	});
});

function mount(): WorkbenchAgentNavigationState {
	return apply(INITIAL_WORKBENCH_AGENT_NAVIGATION_STATE, { kind: 'mount' });
}

function navigate(requestId: number, sessionId: string): WorkbenchAgentNavigationEvent {
	return {
		input: input(sessionId),
		kind: 'navigate',
		originLocation: { kind: 'standalone' },
		requestId,
	};
}

function input(sessionId: string) {
	return {
		projectId: 'project-a',
		sessionId: AgentSessionIdSchema.parse(sessionId),
		workspaceId: 'workspace-a',
	};
}

function projection(value: 'pending' | 'ready' | 'unavailable'): WorkbenchAgentNavigationEvent {
	return {
		activeLocation:
			value === 'ready'
				? { kind: 'workspace', projectId: 'project-a', workspaceId: 'workspace-a' }
				: { kind: 'standalone' },
		kind: 'projection-observed',
		projection: value,
	};
}

function apply(
	state: WorkbenchAgentNavigationState,
	event: WorkbenchAgentNavigationEvent,
): WorkbenchAgentNavigationState {
	return transitionWorkbenchAgentNavigation(state, event).state;
}
