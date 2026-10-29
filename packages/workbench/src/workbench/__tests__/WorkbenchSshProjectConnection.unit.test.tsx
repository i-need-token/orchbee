// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { Workbench } from '../Workbench';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchEnvironmentProjectConnectionCapability,
	WorkbenchEnvironmentProjectConnectionStatus,
} from '../types/workbench-environment-project-connection';

class TestResizeObserver {
	disconnect(): void {}
	observe(_target: Element, _options?: ResizeObserverOptions): void {}
	unobserve(_target: Element): void {}
}

beforeEach(() => vi.stubGlobal('ResizeObserver', TestResizeObserver));
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

describe('Workbench SSH Project connection surface', () => {
	it('adds Desktop enrollment and projects explicit connect, retry, and disconnect actions', async () => {
		const runtime = createRuntime();
		const actionRuntime = new ActionRuntime();
		const connection = createConnectionCapability([
			{ projectId: 'project-1', state: 'disconnected' },
		]);
		const connectViaSsh = vi.fn<() => void>();
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={actionRuntime}
						environmentProjectConnection={connection}
						onConnectViaSsh={connectViaSsh}
						projectWorkspaceRuntime={runtime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		fireEvent.click(screen.getByRole('button', { name: 'Create Project' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Connect via SSH…' }));
		expect(connectViaSsh).toHaveBeenCalledOnce();

		expect(
			screen.getByRole('button', { name: 'Expand project: Remote. Disconnected' }),
		).toBeTruthy();
		await chooseProjectAction('Connect');
		expect(connection.connect).toHaveBeenCalledWith('project-1');

		await act(async () => connection.publish([{ projectId: 'project-1', state: 'connected' }]));
		expect(screen.getByRole('button', { name: 'Expand project: Remote. Connected' })).toBeTruthy();
		await chooseProjectAction('Disconnect');
		expect(connection.disconnect).toHaveBeenCalledWith('project-1');

		await act(async () => connection.publish([{ projectId: 'project-1', state: 'blocked' }]));
		expect(
			screen.getByRole('button', { name: 'Expand project: Remote. Connection needs attention' }),
		).toBeTruthy();
		await chooseProjectAction('Retry Connection');
		expect(connection.connect).toHaveBeenCalledTimes(2);
		actionRuntime.dispose();
	});
});

async function chooseProjectAction(name: string): Promise<void> {
	fireEvent.click(screen.getByRole('button', { name: 'Project options: Remote' }));
	const item = await screen.findByRole('menuitem', { name });
	fireEvent.click(item);
	await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
}

function createConnectionCapability(
	initial: readonly WorkbenchEnvironmentProjectConnectionStatus[],
): WorkbenchEnvironmentProjectConnectionCapability & {
	publish(statuses: readonly WorkbenchEnvironmentProjectConnectionStatus[]): void;
} {
	let statuses = initial;
	const listeners = new Set<() => void>();
	const connect = vi.fn<WorkbenchEnvironmentProjectConnectionCapability['connect']>();
	const disconnect = vi.fn<WorkbenchEnvironmentProjectConnectionCapability['disconnect']>();
	return {
		connect,
		disconnect,
		getSnapshot: () => statuses,
		publish(next) {
			statuses = next;
			for (const listener of listeners) listener();
		},
		subscribe(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}

function createRuntime(): WorkbenchProjectWorkspaceRuntime {
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Terminal 1',
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('ssh-project:initial'),
			() => terminalPresentationId('ssh-project:dynamic'),
		),
	});
	runtime.transition({
		directory: {
			available: false,
			git: null,
			id: 'directory-1',
			kind: 'folder',
			name: 'orchbee',
			path: '/srv/orchbee',
			placement: {
				environmentId: 'environment:00000000000000000000000000000001',
				kind: 'environment',
			},
			source: 'selected',
		},
		projectName: 'Remote',
		type: 'environment-project-registered',
	});
	runtime.transition({
		directoryId: 'directory-1',
		resourceScopeId: 'resource-scope-1',
		projectId: 'project-1',
		type: 'environment-workspace-registered',
		workspaceName: 'Primary',
	});
	return runtime;
}
