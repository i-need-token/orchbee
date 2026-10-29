import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	initializePanelWorkspace,
	transitionPanelWorkspace,
} from '../../components/workbench-panel-workspace/model/panel-workspace-state';
import {
	revalidateCurrentDirectoryTerminalTarget,
	resolveCurrentDirectoryTerminalTarget,
} from '../resolve-current-directory-terminal-target';

describe('current-directory terminal target resolution', () => {
	it('prefers the reported current CWD and falls back to the launch CWD', () => {
		const initial = initialState('/launch');
		expect(resolve(initial)).toMatchObject({
			cwd: '/launch',
			kind: 'ready',
			target: {
				panelId: 'panel-1',
				presentationId: terminalPresentationId('terminal-1'),
				tabId: 'tab-1',
				terminalContextId: 'context-1',
			},
		});
		for (const currentCwd of [null, '']) {
			const withoutCurrentDirectory = transitionPanelWorkspace(initial, {
				currentCwd,
				presentationId: terminalPresentationId('terminal-1'),
				tabId: 'tab-1',
				type: 'terminal-current-cwd-changed',
			}).state;
			expect(resolve(withoutCurrentDirectory)).toMatchObject({ cwd: '/launch', kind: 'ready' });
		}

		const changed = transitionPanelWorkspace(initial, {
			currentCwd: '/reported',
			presentationId: terminalPresentationId('terminal-1'),
			tabId: 'tab-1',
			type: 'terminal-current-cwd-changed',
		}).state;
		expect(resolve(changed)).toMatchObject({ cwd: '/reported', kind: 'ready' });
	});

	it('uses the displayed child and gives an explicit Panel target precedence over focus', () => {
		const split = transitionPanelWorkspace(initialState('/primary'), {
			direction: 'right',
			newTerminalTitle: 'Second',
			panelId: 'panel-1',
			presentation: {
				kind: 'preview',
				presentationId: terminalPresentationId('terminal-2'),
			},
			tabId: 'tab-1',
			type: 'panel-split',
		}).state;
		const child = transitionPanelWorkspace(split, {
			newTerminalTitle: 'Child',
			panelId: 'panel-2',
			presentation: {
				kind: 'preview',
				presentationId: terminalPresentationId('terminal-child'),
			},
			tabId: 'tab-1',
			type: 'child-terminal-created',
		}).state;

		expect(resolve(child)).toMatchObject({
			cwd: '/primary',
			target: { panelId: 'panel-2', presentationId: 'terminal-child' },
		});
		expect(resolve(child, 'panel-1')).toMatchObject({
			cwd: '/primary',
			target: { panelId: 'panel-1', presentationId: 'terminal-1' },
		});
	});

	it('rejects missing active state, missing Panels, and terminals without a usable CWD', () => {
		const state = initialState(null);
		expect(
			resolveCurrentDirectoryTerminalTarget({
				activeTabId: null,
				state,
				targetPanelId: null,
				terminalContextId: 'context-1',
			}),
		).toEqual({ kind: 'unavailable', reason: 'active-tab-unavailable' });
		expect(resolve(state, 'missing')).toEqual({
			kind: 'unavailable',
			reason: 'panel-unavailable',
		});
		expect(resolve(state)).toEqual({
			kind: 'unavailable',
			reason: 'terminal-current-directory-unavailable',
		});
	});

	it('revalidates every captured identity while accepting a newer CWD from the same terminal', () => {
		const initial = initialState('/launch');
		const captured = resolve(initial);
		if (captured.kind !== 'ready') throw new Error('Expected a current-directory target.');
		const changed = transitionPanelWorkspace(initial, {
			currentCwd: '/latest',
			presentationId: captured.target.presentationId,
			tabId: captured.target.tabId,
			type: 'terminal-current-cwd-changed',
		}).state;

		expect(
			revalidateCurrentDirectoryTerminalTarget({
				activeTabId: 'tab-1',
				state: changed,
				target: captured.target,
				terminalContextId: 'context-1',
			}),
		).toMatchObject({ cwd: '/latest', kind: 'ready' });
		for (const target of [
			{ ...captured.target, terminalContextId: 'context-2' },
			{ ...captured.target, tabId: 'tab-2' },
			{ ...captured.target, panelId: 'panel-2' },
			{ ...captured.target, presentationId: terminalPresentationId('terminal-2') },
		]) {
			expect(
				revalidateCurrentDirectoryTerminalTarget({
					activeTabId: 'tab-1',
					state: changed,
					target,
					terminalContextId: 'context-1',
				}),
			).toEqual({ kind: 'unavailable', reason: 'terminal-target-stale' });
		}
	});
});

function initialState(cwd: string | null) {
	return initializePanelWorkspace([
		{
			cwd,
			id: 'tab-1',
			presentation: {
				kind: 'preview',
				presentationId: terminalPresentationId('terminal-1'),
			},
			title: 'Terminal',
		},
	]).state;
}

function resolve(state: ReturnType<typeof initialState>, targetPanelId: string | null = null) {
	return resolveCurrentDirectoryTerminalTarget({
		activeTabId: 'tab-1',
		state,
		targetPanelId,
		terminalContextId: 'context-1',
	});
}
