import { describe, expect, it } from 'vitest';
import {
	INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE,
	transitionWorkspaceCreationActivation as transition,
} from '../workspace-creation-activation';

describe('Workspace creation activation intent', () => {
	it('activates only the latest completed intent after publication', () => {
		let state = transition(INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE, { kind: 'mount' }).state;
		state = transition(state, { kind: 'begin', intent: 1 }).state;
		state = transition(state, { kind: 'begin', intent: 2 }).state;
		state = transition(state, { kind: 'completed', intent: 1, workspaceId: 'old' }).state;
		expect(
			transition(state, { kind: 'projection-observed', workspaceIds: ['old', 'new'] }).commands,
		).toEqual([]);
		state = transition(state, { kind: 'completed', intent: 2, workspaceId: 'new' }).state;
		expect(
			transition(state, { kind: 'projection-observed', workspaceIds: ['old'] }).commands,
		).toEqual([]);
		const completed = transition(state, { kind: 'projection-observed', workspaceIds: ['new'] });
		expect(completed.commands).toEqual([{ kind: 'activate-workspace', workspaceId: 'new' }]);
		expect(
			transition(completed.state, { kind: 'projection-observed', workspaceIds: ['new'] }).commands,
		).toEqual([]);
	});
	it.each(['navigation', 'dispose'] as const)('retires a pending completion on %s', (kind) => {
		let state = transition(INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE, { kind: 'mount' }).state;
		state = transition(state, { kind: 'begin', intent: 1 }).state;
		state = transition(state, { kind }).state;
		state = transition(state, { kind: 'completed', intent: 1, workspaceId: 'new' }).state;
		expect(
			transition(state, { kind: 'projection-observed', workspaceIds: ['new'] }).commands,
		).toEqual([]);
	});
});
