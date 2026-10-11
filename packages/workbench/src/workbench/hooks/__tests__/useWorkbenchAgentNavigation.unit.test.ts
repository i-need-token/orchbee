// @vitest-environment jsdom

import type { ActionExecution } from '@orchbee/actions';
import {
	AgentEventIdSchema,
	AgentRuntime,
	AgentRuntimeSnapshotSchema,
	AgentSessionIdSchema,
} from '@orchbee/agent';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, startTransition, Suspense } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchActiveLocation } from '../../../project-workspace';
import { terminalPresentationId } from '../../../terminal-presentation';
import type { WorkbenchAgentNavigationInput } from '../../model/project-workbench-agent-overview';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { useWorkbenchAgentNavigation } from '../useWorkbenchAgentNavigation';
import { pendingActionExecution } from './pendingActionExecution';

afterEach(cleanup);

const blockedExecution: ActionExecution = {
	cancel: () => undefined,
	completion: Promise.resolve({ code: 'unavailable', kind: 'blocked' }),
	executionId: 'agent-navigation-test',
	getSnapshot: () => ({
		progress: null,
		result: { code: 'unavailable', kind: 'blocked' },
		state: 'blocked',
	}),
	subscribe: () => ({ dispose: () => undefined }),
};

describe('useWorkbenchAgentNavigation', () => {
	it.each([false, true])(
		'cancels pending focus only for committed unrelated navigation (committed: %s)',
		async (committed) => {
			const fixture = navigationFixture();
			const execution = pendingActionExecution('pending-agent-navigation');
			const announceUnavailable = vi.fn<() => void>();
			const requestTerminalFocus = vi.fn<() => boolean>(() => true);
			const invokeAgentNavigation = vi.fn<
				(input: WorkbenchAgentNavigationInput) => ActionExecution
			>(() => execution.execution);
			const origin: WorkbenchActiveLocation = { kind: 'standalone' };
			const unrelated: WorkbenchActiveLocation = {
				kind: 'workspace',
				workspaceId: fixture.unrelatedWorkspaceId,
			};
			const pending = new Promise<void>(() => undefined);
			const initialProps: { activeLocation: WorkbenchActiveLocation; suspend: boolean } = {
				activeLocation: origin,
				suspend: false,
			};
			const rendered = renderHook(
				({ activeLocation, suspend }) => {
					const navigate = useWorkbenchAgentNavigation({
						activeLocation,
						announceUnavailable,
						invokeAgentNavigation,
						projectWorkspaceRuntime: fixture.runtime,
						requestTerminalFocus,
						source: fixture.source,
					});
					if (suspend) throw pending;
					return navigate;
				},
				{
					initialProps,
					wrapper: ({ children }) => createElement(Suspense, { fallback: null }, children),
				},
			);
			if (!committed) {
				act(() =>
					startTransition(() => {
						rendered.rerender({ activeLocation: unrelated, suspend: true });
					}),
				);
			}
			act(() => rendered.result.current(fixture.input));
			if (committed) {
				act(() => {
					fixture.runtime.transition({
						type: 'workspace-activated',
						workspaceId: fixture.unrelatedWorkspaceId,
					});
				});
			}
			rendered.rerender({ activeLocation: committed ? unrelated : origin, suspend: false });
			act(() => {
				const observed = fixture.source.dispatch({
					eventId: AgentEventIdSchema.parse('navigation-observation-stale'),
					generation: 0,
					sequence: 2,
					sessionId: fixture.input.sessionId,
					type: 'observation-became-stale',
				});
				expect(observed.kind).toBe('applied');
			});
			expect(invokeAgentNavigation).toHaveBeenCalledExactlyOnceWith(fixture.input);
			expect(requestTerminalFocus).not.toHaveBeenCalled();
			act(() => {
				fixture.runtime.transition({
					type: 'workspace-activated',
					workspaceId: fixture.input.workspaceId,
				});
			});
			rendered.rerender({
				activeLocation: { kind: 'workspace', workspaceId: fixture.input.workspaceId },
				suspend: false,
			});
			await execution.settle({ kind: 'completed' });
			expect(announceUnavailable).not.toHaveBeenCalled();
			expect(requestTerminalFocus).toHaveBeenCalledTimes(committed ? 0 : 1);
			rendered.unmount();
			fixture.source.dispose();
		},
	);

	it('announces a Session that is unavailable in the latest projection', () => {
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal',
			terminalPresentation: {
				createPresentationId: () => terminalPresentationId('agent-navigation-dynamic'),
				initialPresentationId: terminalPresentationId('agent-navigation-initial'),
				kind: 'preview',
			},
		});
		const announceUnavailable = vi.fn<() => void>();
		const invokeAgentNavigation = vi.fn<(input: WorkbenchAgentNavigationInput) => ActionExecution>(
			() => blockedExecution,
		);
		const requestTerminalFocus = vi.fn<() => boolean>(() => true);
		const rendered = renderHook(() =>
			useWorkbenchAgentNavigation({
				activeLocation: { kind: 'standalone' },
				announceUnavailable,
				invokeAgentNavigation,
				projectWorkspaceRuntime: runtime,
				requestTerminalFocus,
				source: undefined,
			}),
		);

		act(() =>
			rendered.result.current({
				sessionId: AgentSessionIdSchema.parse('session-a'),
				workspaceId: 'workspace-a',
			}),
		);

		expect(invokeAgentNavigation).toHaveBeenCalledOnce();
		expect(announceUnavailable).toHaveBeenCalledOnce();
		expect(requestTerminalFocus).not.toHaveBeenCalled();
	});
});

function navigationFixture() {
	let presentationSequence = 0;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Terminal',
		terminalPresentation: {
			createPresentationId: () => terminalPresentationId(`navigation-${++presentationSequence}`),
			initialPresentationId: terminalPresentationId('navigation-standalone'),
			kind: 'preview',
		},
	});
	runtime.transition({
		directory: {
			available: true,
			git: null,
			id: 'navigation-directory',
			kind: 'folder',
			name: 'Navigation',
			path: '/navigation',
			placement: { kind: 'local' },
			source: 'selected',
		},
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: 'Target Workspace',
		projectName: 'Navigation',
		terminalTitle: 'Target Terminal',
		type: 'project-created',
	});
	const project = runtime.getSnapshot().projects[0];
	const target = runtime.getSnapshot().workspaces[0];
	if (project === undefined || target === undefined)
		throw new Error('Expected a target Workspace.');
	const panel = runtime
		.getTerminalContext(target.terminalContextId)
		?.panelWorkspaceStore.getSnapshot().tabs[0]?.root;
	if (panel?.kind !== 'panel') throw new Error('Expected one target Panel.');
	runtime.transition({
		directoryId: target.primaryDirectoryId,
		launchConfigurationId: null,
		name: 'Unrelated Workspace',
		projectId: project.id,
		resourceScopeId: target.resourceScopeId,
		terminalTitle: 'Unrelated Terminal',
		type: 'workspace-created',
	});
	const unrelated = runtime
		.getSnapshot()
		.workspaces.find((workspace) => workspace.id !== target.id);
	if (unrelated === undefined) throw new Error('Expected an unrelated Workspace.');
	runtime.transition({ type: 'standalone-activated' });
	const sessionId = AgentSessionIdSchema.parse('navigation-session');
	const source = new AgentRuntime(
		AgentRuntimeSnapshotSchema.parse({
			revision: 1,
			sessions: [
				{
					activityHint: null,
					capabilities: [],
					executionLocation: { executionLocationId: 'navigation-execution' },
					lastOutcome: null,
					lifecycle: 'live',
					observation: {
						fidelity: 'rich',
						freshness: 'fresh',
						generation: 0,
						sequence: 1,
						source: 'hook',
					},
					parentSessionId: null,
					pendingRequests: [],
					presentationAttachments: [
						{
							generation: 'navigation-generation',
							navigable: true,
							presentationId: panel.primaryPresentation.presentationId,
							visibility: 'visible',
						},
					],
					providerId: 'codex',
					sessionId,
					turn: { kind: 'idle' },
				},
			],
		}),
	);
	return {
		input: { sessionId, workspaceId: target.id },
		runtime,
		source,
		unrelatedWorkspaceId: unrelated.id,
	};
}
