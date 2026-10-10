// @vitest-environment jsdom

import type { ActionExecution } from '@orchbee/actions';
import { AgentSessionIdSchema } from '@orchbee/agent';
import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import type { WorkbenchAgentNavigationInput } from '../../model/project-workbench-agent-overview';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { useWorkbenchAgentNavigation } from '../useWorkbenchAgentNavigation';

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
