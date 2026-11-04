// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useEffect, useLayoutEffect, useRef, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench-client-protocol';

import { Workbench } from '../Workbench';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchNavigationPort } from '../types/workbench-host-actions';
import type { WorkbenchNewProjectFolderCapability } from '../types/workbench-project-folder-acquisition';
import type { WorkbenchAgentSnapshot } from '../types/workbench-agent-observation';
import type { WorkbenchTerminalSurfaceProps } from '../types/workbench-terminal-surface';
import { findActiveTerminalFocusTarget } from '../utils/findActiveTerminalFocusTarget';

function createTestTerminalPresentationComposition(scope: string) {
	let sequence = 0;
	return createWorkbenchPreviewTerminalPresentationComposition(
		terminalPresentationId(`${scope}:initial`),
		() => terminalPresentationId(`${scope}:dynamic:${(sequence += 1)}`),
	);
}

function createTestManagedTerminalPresentationComposition(scope: string) {
	let sequence = 0;
	return {
		createOperationId: () =>
			terminalPresentationOperationId(`${scope}:operation:${(sequence += 1)}`),
		createPresentationId: () => terminalPresentationId(`${scope}:presentation:${(sequence += 1)}`),
		createRemovalId: () => terminalPresentationRemovalId(`${scope}:removal:${(sequence += 1)}`),
		dispatchCommands: () => undefined,
		initialPresentationId: terminalPresentationId(`${scope}:initial`),
		kind: 'managed' as const,
	};
}

function FocusTestTerminalSurface({
	autoFocus,
	focusRequest,
	onPresentationReady,
	presentation,
	presentationMode,
}: WorkbenchTerminalSurfaceProps): ReactNode {
	const inputReference = useRef<HTMLTextAreaElement>(null);
	useLayoutEffect(() => {
		if (presentationMode !== 'incoming') return;
		onPresentationReady('display-ready');
		onPresentationReady('activation-ready');
	}, [onPresentationReady, presentationMode]);
	useEffect(() => {
		if (autoFocus || focusRequest > 0) inputReference.current?.focus();
	}, [autoFocus, focusRequest, presentation.presentationId]);
	return (
		<textarea
			aria-label={`Terminal input: ${presentation.title}`}
			data-test-terminal-input={presentation.presentationId}
			ref={inputReference}
		/>
	);
}

function createTerminalFocusProjectWorkspaceRuntime(scope: string) {
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone Terminal',
		terminalPresentation: createTestTerminalPresentationComposition(scope),
	});
	runtime.transition({
		directory: {
			available: true,
			git: null,
			id: 'directory-orchbee',
			kind: 'folder',
			name: 'orchbee',
			path: '/workspace/orchbee',
			placement: { kind: 'local' },
			source: 'selected',
		},
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: 'Primary',
		projectName: 'orchbee',
		terminalTitle: 'Workspace Terminal',
		type: 'project-created',
	});
	return runtime;
}

class TestResizeObserver {
	public disconnect(): void {}

	public observe(_target: Element, _options?: ResizeObserverOptions): void {}

	public unobserve(_target: Element): void {}
}

beforeEach(() => {
	vi.stubGlobal('ResizeObserver', TestResizeObserver);
});

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

function expectActionableMenuItemsToHaveIcons(menu: HTMLElement): void {
	for (const menuItem of within(menu).getAllByRole('menuitem')) {
		const icon = menuItem.querySelector('[data-icon]');
		expect(icon).not.toBeNull();
		expect(icon?.getAttribute('data-icon-size')).toBe('menu-leading');
		expect(menuItem.className).toContain('[&_[data-icon]]:text-muted-foreground');
	}
}

function getActionableMenuPresentation(menu: HTMLElement): readonly {
	readonly icon: string | null;
	readonly label: string | null;
}[] {
	return within(menu)
		.getAllByRole('menuitem')
		.map((menuItem) => ({
			icon: menuItem.querySelector('[data-icon]')?.getAttribute('data-icon') ?? null,
			label: menuItem.textContent,
		}));
}

describe('Workbench Project and Workspace management', () => {
	it('opens exact row menus from both menu keys without activating same-named Workspaces', async () => {
		const projectWorkspaceRuntime = createTerminalFocusProjectWorkspaceRuntime('row-menu-keys');
		projectWorkspaceRuntime.transition({
			directoryId: 'directory-orchbee',
			launchConfigurationId: null,
			name: 'Primary',
			resourceScopeId: 'resource-scope-1',
			projectId: 'project-1',
			terminalTitle: 'Other terminal',
			type: 'workspace-created',
		});
		const { container } = render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='standard'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);
		const workspace = container.querySelector<HTMLElement>('[data-workspace-id="workspace-1"]');
		if (workspace === null) throw new Error('Expected the inactive first Workspace.');
		const workspaceButton = within(workspace).getByRole('button', { name: 'Primary' });
		const workspaceOptions = within(workspace).getByRole('button', {
			name: 'Workspace options: Primary',
		});
		const projectButton = screen.getByRole('button', { name: 'Collapse project: orchbee' });
		const projectOptions = screen.getByRole('button', { name: 'Project options: orchbee' });
		const snapshot = projectWorkspaceRuntime.getSnapshot();
		for (const [button, options, command] of [
			[projectButton, projectOptions, 'Dissolve'],
			[workspaceButton, workspaceOptions, 'Move to Project'],
		] as const) {
			for (const [chord, nativeContextTarget] of [
				[{ code: 'F10', key: 'F10', shiftKey: true }, null],
				[{ code: 'ContextMenu', key: 'ContextMenu' }, 'row'],
				[{ code: 'ContextMenu', key: 'ContextMenu' }, 'popup'],
			] as const) {
				act(() => button.focus());
				const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...chord });
				fireEvent(button, event);
				expect(event.defaultPrevented).toBe(true);
				if (nativeContextTarget === 'row') {
					fireEvent.keyUp(button, chord);
					fireEvent.contextMenu(button, { button: -1 });
				}
				const menu = await screen.findByRole('menu');
				expect(menu.getAttribute('data-slot')).toBe('dropdown-menu-content');
				await waitFor(() => expect(menu.contains(document.activeElement)).toBe(true));
				const menuFocusTarget = document.activeElement;
				if (menuFocusTarget === null) throw new Error('Expected the focused portaled menu target.');
				expect(options.getAttribute('aria-controls')).toBe(menu.id);
				expect(
					options.closest('[data-slot="context-menu-trigger"]')?.contains(menuFocusTarget),
				).toBe(false);
				if (nativeContextTarget === 'popup') {
					fireEvent.keyUp(menuFocusTarget, chord);
					fireEvent.contextMenu(menuFocusTarget, { button: -1 });
				}
				expect(within(menu).getByRole('menuitem', { name: command })).toBeDefined();
				expect(screen.getAllByRole('menu')).toHaveLength(1);
				expect(projectWorkspaceRuntime.getSnapshot()).toBe(snapshot);
				fireEvent.keyDown(menu, { code: 'F10', key: 'F10', shiftKey: true });
				expect(screen.getAllByRole('menu')).toHaveLength(1);
				fireEvent.keyDown(document, { key: 'Escape' });
				await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
				await waitFor(() => expect(document.activeElement).toBe(options));
				expect(projectWorkspaceRuntime.getSnapshot()).toBe(snapshot);
			}
		}
		act(() => workspaceButton.focus());
		fireEvent.keyDown(workspaceButton, { code: 'ContextMenu', key: 'ContextMenu' });
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Rename' }));
		const dialog = await screen.findByRole('dialog');
		const nameInput = within(dialog).getByLabelText('Rename Workspace');
		await waitFor(() => expect(document.activeElement).toBe(nameInput));
		fireEvent.change(nameInput, { target: { value: 'First only' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Rename' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(
			projectWorkspaceRuntime.getSnapshot().workspaces.map(({ id, name }) => ({ id, name })),
		).toEqual([
			{ id: 'workspace-1', name: 'First only' },
			{ id: 'workspace-2', name: 'Primary' },
		]);
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toBe(snapshot.activeLocation);
	});

	it('leaves composing, repeated, modified and terminal-owned menu keys unconsumed', () => {
		const projectWorkspaceRuntime = createTerminalFocusProjectWorkspaceRuntime('row-menu-guards');
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='standard'
						terminalSurface={FocusTestTerminalSurface}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);
		const workspaceButton = screen.getByRole('button', { name: 'Primary' });
		for (const chord of [
			{ code: 'F10', shiftKey: false },
			{ code: 'F10', shiftKey: true, ctrlKey: true },
			{ code: 'F10', shiftKey: true, altKey: true },
			{ code: 'F10', shiftKey: true, metaKey: true },
			{ code: 'F10', shiftKey: true, repeat: true },
			{ code: 'F10', shiftKey: true, isComposing: true },
			{ code: 'F10', shiftKey: true, keyCode: 229 },
			{ code: 'ContextMenu', shiftKey: true },
			{ code: 'ContextMenu', ctrlKey: true },
			{ code: 'ContextMenu', altKey: true },
			{ code: 'ContextMenu', metaKey: true },
			{ code: 'ContextMenu', repeat: true },
			{ code: 'ContextMenu', isComposing: true },
		]) {
			const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...chord });
			fireEvent(workspaceButton, event);
			expect(event.defaultPrevented).toBe(false);
			expect(screen.queryByRole('menu')).toBeNull();
		}
		const terminal = screen.getByRole('textbox', { name: 'Terminal input: Workspace Terminal' });
		for (const chord of [{ code: 'F10', shiftKey: true }, { code: 'ContextMenu' }]) {
			const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...chord });
			fireEvent(terminal, event);
			expect(event.defaultPrevented).toBe(false);
			expect(screen.queryByRole('menu')).toBeNull();
		}
	});

	it('returns context-menu focus to the row and respects outside, dialog and removal handoffs', async () => {
		const projectWorkspaceRuntime = createTerminalFocusProjectWorkspaceRuntime('row-menu-return');
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='standard'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);
		const workspaceButton = screen.getByRole('button', { name: 'Primary' });
		const options = screen.getByRole('button', { name: 'Workspace options: Primary' });
		const outside = screen.getByRole('button', { name: 'Collapse project: orchbee' });
		const snapshot = projectWorkspaceRuntime.getSnapshot();
		act(() => outside.focus());
		fireEvent.contextMenu(workspaceButton, { clientX: 120, clientY: 180 });
		await screen.findByRole('menu');
		fireEvent.keyDown(document, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		await waitFor(() => expect(document.activeElement).toBe(options));
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(snapshot);

		fireEvent.contextMenu(workspaceButton, { clientX: 120, clientY: 180 });
		await screen.findByRole('menu');
		fireEvent.mouseDown(outside);
		act(() => outside.focus());
		fireEvent.mouseUp(outside);
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		expect(document.activeElement).toBe(outside);
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(snapshot);

		fireEvent.contextMenu(workspaceButton, { clientX: 120, clientY: 180 });
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Rename' }));
		const dialog = await screen.findByRole('dialog');
		await waitFor(() =>
			expect(document.activeElement).toBe(within(dialog).getByLabelText('Rename Workspace')),
		);
		fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(snapshot);

		fireEvent.contextMenu(workspaceButton, { clientX: 120, clientY: 180 });
		const menu = await screen.findByRole('menu');
		const staleCommand = within(menu).getByRole('menuitem', { name: 'Close' });
		act(() => {
			projectWorkspaceRuntime.transition({
				type: 'workspace-deleted',
				workspaceId: 'workspace-1',
			});
		});
		act(() => outside.focus());
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		const afterRemoval = projectWorkspaceRuntime.getSnapshot();
		fireEvent.click(staleCommand);
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(afterRemoval);
		expect(document.activeElement).toBe(outside);
		expect(options.isConnected).toBe(false);
	});

	it('creates independently, groups through the Workspace menu, and dissolves without touching terminals', async () => {
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/home',
			standaloneTerminalTitle: 'Shell',
			terminalPresentation: createTestTerminalPresentationComposition('organization-ui'),
		});
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={runtime}
						shortcutPlatform='standard'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);
		const create = screen.getByRole('button', { name: 'New workspace' });
		const fixedActions = create.closest('[data-slot="workbench-sidebar-actions"]');
		if (!(fixedActions instanceof HTMLElement)) throw new Error('Expected fixed sidebar actions.');
		expect(
			within(fixedActions)
				.getAllByRole('button')
				.map((button) => button.textContent),
		).toEqual(['New workspace', expect.stringContaining('Search')]);
		expect(fixedActions.closest('[data-navigation-reorder-scroll]')).toBeNull();
		// The list has no heading; only a group with Workspaces shows its own.
		expect(screen.getByRole('list', { name: 'Workspaces' })).toBeTruthy();
		expect(screen.queryByRole('heading', { name: /Workspaces/u })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Collapse Recents' })).toBeNull();
		fireEvent.click(create);
		await waitFor(() => expect(runtime.getSnapshot().workspaces).toHaveLength(1));
		fireEvent.click(screen.getByRole('button', { name: 'Collapse Recents' }));
		await waitFor(() => expect(screen.queryByRole('button', { name: 'Workspace' })).toBeNull());
		fireEvent.click(screen.getByRole('button', { name: 'Expand Recents' }));
		await screen.findByRole('button', { name: 'Workspace' });
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(runtime.getSnapshot().projects).toEqual([]);
		const workspace = runtime.getSnapshot().workspaces[0];
		if (workspace === undefined) throw new Error('Expected the new Workspace.');
		expect(workspace).toMatchObject({
			name: 'Workspace',
			projectId: null,
			launchConfigurationId: null,
		});
		const context = runtime.getActiveTerminalContext();
		const topology = context.panelWorkspaceStore.getSnapshot();
		expect(topology.tabs[0]?.root).toMatchObject({
			kind: 'panel',
			primaryPresentation: { launchCwd: '/home' },
		});
		const openMove = async () => {
			fireEvent.contextMenu(screen.getByRole('button', { name: 'Workspace' }));
			fireEvent.click(await screen.findByRole('menuitem', { name: 'Move to Project' }));
		};
		await openMove();
		fireEvent.click(await screen.findByRole('menuitem', { name: 'New Project…' }));
		const dialog = await screen.findByRole('dialog');
		fireEvent.change(within(dialog).getByLabelText('New Project and Move'), {
			target: { value: 'Review' },
		});
		fireEvent.click(within(dialog).getByRole('button', { name: 'Create' }));
		await waitFor(() => expect(runtime.getSnapshot().workspaces[0]?.projectId).toBe('project-1'));
		expect(runtime.getActiveTerminalContext()).toBe(context);
		expect(context.panelWorkspaceStore.getSnapshot()).toBe(topology);
		act(() => runtime.transition({ type: 'project-expansion-toggled', projectId: 'project-1' }));
		await openMove();
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Remove from Project' }));
		await waitFor(() => expect(runtime.getSnapshot().workspaces[0]?.projectId).toBeNull());
		await openMove();
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Review' }));
		await waitFor(() => expect(runtime.getSnapshot().workspaces[0]?.projectId).toBe('project-1'));
		fireEvent.click(screen.getByRole('button', { name: 'Project options: Review' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Dissolve' }));
		await waitFor(() => expect(runtime.getSnapshot().projects).toEqual([]));
		expect(runtime.getSnapshot().workspaces).toEqual([workspace]);
		expect(runtime.getActiveTerminalContext()).toBe(context);
		expect(context.panelWorkspaceStore.getSnapshot()).toBe(topology);
		expect(screen.queryByRole('alertdialog')).toBeNull();
	});

	it('lists observed Workspace Agents until folded, keeps the fold, and opens the exact terminal', async () => {
		const projectWorkspaceRuntime = createTerminalFocusProjectWorkspaceRuntime(
			'workspace-agent-overview',
		);
		const workspace = projectWorkspaceRuntime
			.getSnapshot()
			.workspaces.find(
				(entry) => entry.projectId === projectWorkspaceRuntime.getSnapshot().projects[0]?.id,
			);
		if (workspace === undefined) throw new Error('Expected the primary Workspace.');
		const context = projectWorkspaceRuntime.getTerminalContext(workspace.terminalContextId);
		if (context === null) throw new Error('Expected the Workspace terminal context.');
		const creation = projectWorkspaceRuntime.createTerminal({
			groupId: null,
			source: 'programmatic',
			title: 'Agent Tab',
		});
		if (creation.kind !== 'created') throw new Error('Expected the Agent Tab.');
		const targetTab = context.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((tab) => tab.tabId === creation.tabId);
		if (targetTab?.root.kind !== 'panel') throw new Error('Expected the Agent Panel.');
		const presentationId = targetTab.root.primaryPresentation.presentationId;
		const targetTabTitle = context.tabModel
			.getSnapshot()
			.tabs.find((tab) => tab.id === creation.tabId)?.title;
		if (targetTabTitle === undefined) throw new Error('Expected the Agent Tab title.');
		let agents: WorkbenchAgentSnapshot = {
			complete: true,
			presentations: new Map([
				[presentationId, { agent: 'codex', binding: 'terminal', sequence: 1n, state: 'idle' }],
			]),
		};
		const agentListeners = new Set<() => void>();
		const agentSource = {
			getSnapshot: () => agents,
			subscribe: (listener: () => void) => {
				agentListeners.add(listener);
				return () => {
					agentListeners.delete(listener);
				};
			},
		};
		projectWorkspaceRuntime.transition({ type: 'standalone-activated' });

		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						agentObservationSource={agentSource}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
						terminalSurface={FocusTestTerminalSurface}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		// As in the TUI, a Workspace lists its Agents until this Client folds them away.
		const workspaceButton = screen.getByRole('button', {
			name: 'Primary. 1 live Agent. Codex: Ready',
		});
		expect(workspaceButton.querySelector('[data-agent-provider-icon="codex"]')).not.toBeNull();
		const agentName = `Codex, Ready, ${targetTabTitle} · Panel 1 · Primary terminal`;
		const agentButton = screen.getByRole('button', { name: agentName });
		expect(agentButton.textContent).toContain('Codex');
		expect(agentButton.textContent).toContain(`${targetTabTitle} · Panel 1 · Primary terminal`);

		// Folded, the row's second line lists the states, and the fold outlasts a folded Project and a
		// hidden sidebar.
		fireEvent.click(screen.getByRole('button', { name: 'Hide 1 Agent in Primary' }));
		expect(screen.queryByRole('button', { name: agentName })).toBeNull();
		const states = workspaceButton.querySelector(
			'[data-slot="workbench-sidebar-workspace-agent-states"]',
		);
		expect(states?.getAttribute('title')).toBe('Ready');
		expect(states?.querySelectorAll('[data-agent-state-glyph]')).toHaveLength(1);
		fireEvent.click(screen.getByRole('button', { name: 'Collapse project: orchbee' }));
		fireEvent.click(screen.getByRole('button', { name: 'Expand project: orchbee' }));
		fireEvent.click(screen.getByRole('button', { name: 'Hide Sidebar' }));
		fireEvent.click(screen.getByRole('button', { name: 'Show Sidebar' }));
		expect(screen.queryByRole('button', { name: agentName })).toBeNull();
		fireEvent.click(screen.getByRole('button', { name: 'Show 1 Agent in Primary' }));
		fireEvent.click(screen.getByRole('button', { name: agentName }));

		await waitFor(() => {
			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',

				workspaceId: workspace.id,
			});
			expect(context.tabModel.getSnapshot().activeTabId).toBe(creation.tabId);
			expect(document.activeElement).toBe(
				document.querySelector(`[data-test-terminal-input="${presentationId}"]`),
			);
		});
		// The Agent exited, so the Server reports none in the terminal.
		act(() => {
			agents = { complete: true, presentations: new Map() };
			for (const listener of agentListeners) listener();
		});
		await waitFor(() => {
			expect(screen.queryByRole('button', { name: /Codex, Ready/iu })).toBeNull();
			expect(screen.queryByRole('button', { name: /Agent in Primary/iu })).toBeNull();
			expect(screen.getByRole('button', { name: 'Primary' }).textContent).toContain('Primary');
		});
	});

	it('focuses the standalone terminal only after pointer-click navigation is published', async () => {
		const projectWorkspaceRuntime = createTerminalFocusProjectWorkspaceRuntime('pointer-focus');
		const navigate = vi.fn<WorkbenchNavigationPort['navigate']>((context) => {
			context.markCommitted();
			return { kind: 'completed' };
		});
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						navigation={{ navigate, selectTab: () => ({ kind: 'unchanged' }) }}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
						terminalSurface={FocusTestTerminalSurface}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		const terminalButton = screen.getByRole('button', { name: 'Terminal' });
		act(() => terminalButton.focus());
		expect(document.activeElement).toBe(terminalButton);
		fireEvent.click(terminalButton, { detail: 1 });
		await waitFor(() =>
			expect(navigate).toHaveBeenCalledWith(expect.anything(), { kind: 'standalone' }),
		);
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation.kind).toBe('workspace');
		expect(document.activeElement).toBe(terminalButton);

		act(() => {
			projectWorkspaceRuntime.transition({ type: 'standalone-activated' });
		});
		await waitFor(() => {
			const standaloneInput = document.querySelector<HTMLTextAreaElement>(
				'[data-test-terminal-input="pointer-focus:initial"]',
			);
			expect(standaloneInput).not.toBeNull();
			expect(document.activeElement).toBe(standaloneInput);
		});
	});

	it('keeps keyboard activation of the Terminal button focus-neutral', async () => {
		const projectWorkspaceRuntime = createTerminalFocusProjectWorkspaceRuntime('keyboard-focus');
		const navigate = vi.fn<WorkbenchNavigationPort['navigate']>((context) => {
			context.markCommitted();
			return { kind: 'completed' };
		});
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						navigation={{ navigate, selectTab: () => ({ kind: 'unchanged' }) }}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
						terminalSurface={FocusTestTerminalSurface}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		const terminalButton = screen.getByRole('button', { name: 'Terminal' });
		act(() => terminalButton.focus());
		fireEvent.click(terminalButton, { detail: 0 });
		await waitFor(() => expect(navigate).toHaveBeenCalledOnce());
		act(() => {
			projectWorkspaceRuntime.transition({ type: 'standalone-activated' });
		});
		await waitFor(() =>
			expect(
				document.querySelector('[data-test-terminal-input="keyboard-focus:initial"]'),
			).not.toBeNull(),
		);
		expect(document.activeElement).toBe(terminalButton);
	});

	it('focuses the Workspace terminal only after pointer-click navigation is published', async () => {
		const projectWorkspaceRuntime = createTerminalFocusProjectWorkspaceRuntime('workspace-focus');
		projectWorkspaceRuntime.transition({ type: 'standalone-activated' });
		const navigate = vi.fn<WorkbenchNavigationPort['navigate']>((context) => {
			context.markCommitted();
			return { kind: 'completed' };
		});
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						navigation={{ navigate, selectTab: () => ({ kind: 'unchanged' }) }}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
						terminalSurface={FocusTestTerminalSurface}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		const workspaceButton = screen.getByRole('button', { name: 'Primary' });
		act(() => workspaceButton.focus());
		fireEvent.click(workspaceButton, { detail: 1 });
		await waitFor(() =>
			expect(navigate).toHaveBeenCalledWith(expect.anything(), {
				kind: 'workspace',

				workspaceId: 'workspace-1',
			}),
		);
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation.kind).toBe('standalone');
		expect(document.activeElement).toBe(workspaceButton);

		act(() => {
			projectWorkspaceRuntime.transition({
				type: 'workspace-activated',
				workspaceId: 'workspace-1',
			});
		});
		await waitFor(() =>
			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',

				workspaceId: 'workspace-1',
			}),
		);
		await waitFor(() => {
			const workspaceInput = screen.getByRole('textbox', {
				name: 'Terminal input: Workspace Terminal',
			});
			expect(document.activeElement).toBe(workspaceInput);
		});
	});

	it('keeps keyboard activation of a Workspace button focus-neutral', async () => {
		const projectWorkspaceRuntime = createTerminalFocusProjectWorkspaceRuntime(
			'workspace-keyboard-focus',
		);
		projectWorkspaceRuntime.transition({ type: 'standalone-activated' });
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
						terminalSurface={FocusTestTerminalSurface}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		const workspaceButton = screen.getByRole('button', { name: 'Primary' });
		act(() => workspaceButton.focus());
		fireEvent.click(workspaceButton, { detail: 0 });
		await waitFor(() =>
			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',

				workspaceId: 'workspace-1',
			}),
		);
		await waitFor(() =>
			expect(
				screen.getByRole('textbox', { name: 'Terminal input: Workspace Terminal' }),
			).not.toBeNull(),
		);
		expect(document.activeElement).toBe(workspaceButton);
	});

	it('adapts New Tab through exact terminal creation and selects only its returned Tab', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('exact-action'),
		});
		for (const [index, name] of ['alpha', 'beta'].entries()) {
			projectWorkspaceRuntime.transition({
				directory: {
					available: true,
					git: null,
					id: `directory-${name}`,
					kind: 'folder',
					name,
					path: `/projects/${name}`,
					placement: { kind: 'local' },
					source: 'selected',
				},
				launchConfigurations: [],
				primaryWorkspaceLaunchConfigurationId: null,
				primaryWorkspaceName: `${name} workspace`,
				projectName: name,
				terminalTitle: `Terminal ${index + 1}`,
				type: 'project-created',
			});
		}
		const [alphaWorkspace, betaWorkspace] = projectWorkspaceRuntime.getSnapshot().workspaces;
		const alphaContext =
			alphaWorkspace === undefined
				? null
				: projectWorkspaceRuntime.getTerminalContext(alphaWorkspace.terminalContextId);
		const betaContext =
			betaWorkspace === undefined
				? null
				: projectWorkspaceRuntime.getTerminalContext(betaWorkspace.terminalContextId);
		if (alphaContext === null || betaContext === null) {
			throw new Error('Expected both Project terminal contexts.');
		}
		const alphaSelection = alphaContext.tabModel.getSnapshot().activeTabId;

		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		fireEvent.click(screen.getByRole('button', { name: /new tab/iu }));
		await waitFor(() => expect(betaContext.tabModel.getSnapshot().tabs).toHaveLength(2));

		expect(alphaContext.tabModel.getSnapshot()).toMatchObject({
			activeTabId: alphaSelection,
			tabs: [{ id: alphaSelection }],
		});
		expect(betaContext.tabModel.getSnapshot().activeTabId).toBe('terminal-context-3-tab-2');
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',

			workspaceId: 'workspace-2',
		});
	});

	it('renames and restores the captured Tab title from the shortcut and context menu', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('tab-title'),
		});
		const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
		const firstTabId = terminalContext.tabModel.getSnapshot().activeTabId;
		const creation = projectWorkspaceRuntime.createTerminal({
			groupId: null,
			source: 'button',
			title: 'Terminal 2',
		});
		if (firstTabId === null || creation.kind !== 'created') {
			throw new Error('Expected two terminal Tabs.');
		}
		terminalContext.tabModel.selectTab(firstTabId);
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);
		const activeViewport = document.querySelector<HTMLElement>(
			'[data-slot="workbench-terminal-viewport"]',
		);
		expect(activeViewport).not.toBeNull();
		if (activeViewport === null) throw new Error('Expected the active terminal viewport.');
		activeViewport.focus();
		activeViewport.dispatchEvent(
			new KeyboardEvent('keydown', {
				bubbles: true,
				cancelable: true,
				code: 'KeyI',
				metaKey: true,
				shiftKey: true,
			}),
		);

		let dialog = await screen.findByRole('dialog', { name: 'Rename Tab' });
		expect(dialog.getAttribute('data-slot')).toBe('workbench-name-dialog');
		// The title names the only field; a local blank name returns to the terminal's title.
		let titleInput = within(dialog).getByLabelText<HTMLInputElement>('Rename Tab');
		expect(titleInput.getAttribute('aria-required')).toBeNull();
		expect(titleInput.getAttribute('aria-describedby')).not.toBeNull();
		const titleCancel = within(dialog).getByRole('button', { name: 'Cancel' });
		const titleSave = within(dialog).getByRole('button', { name: 'Rename' });
		expect(titleInput.value).toBe('system');
		await waitFor(() => expect(document.activeElement).toBe(titleInput));
		expect(titleInput.selectionStart).toBe(0);
		expect(titleInput.selectionEnd).toBe(titleInput.value.length);
		expect(within(titleCancel).getByText('Esc')).toBeDefined();
		expect(within(titleSave).getByText('Enter')).toBeDefined();

		terminalContext.tabModel.selectTab(creation.tabId);
		fireEvent.change(titleInput, { target: { value: '  Focus work  ' } });
		let form = dialog.querySelector('form');
		expect(form).not.toBeNull();
		if (form === null) throw new Error('Expected the Tab title form.');
		fireEvent.submit(form);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(
			terminalContext.tabModel.getSnapshot().tabs.find((tab) => tab.id === firstTabId)?.title,
		).toBe('Focus work');
		expect(
			terminalContext.tabModel.getSnapshot().tabs.find((tab) => tab.id === creation.tabId)?.title,
		).not.toBe('Focus work');

		const titleReturnFocus = findActiveTerminalFocusTarget(projectWorkspaceRuntime);
		expect(titleReturnFocus).not.toBeNull();
		const firstTab = screen.getByRole('tab', { name: /^Focus work/ });
		fireEvent.contextMenu(firstTab, { clientX: 120, clientY: 20 });
		const menu = await screen.findByRole('menu', { name: 'Tab options' });
		const titleMenuItem = within(menu).getByRole('menuitem', { name: 'Rename' });
		expect(titleMenuItem.textContent).toBe('Rename');
		fireEvent.click(titleMenuItem);
		dialog = await screen.findByRole('dialog', { name: 'Rename Tab' });
		titleInput = within(dialog).getByLabelText<HTMLInputElement>('Rename Tab');
		expect(titleInput.value).toBe('Focus work');
		await waitFor(() => expect(document.activeElement).toBe(titleInput));
		fireEvent.change(titleInput, { target: { value: '   ' } });
		form = dialog.querySelector('form');
		expect(form).not.toBeNull();
		if (form === null) throw new Error('Expected the Tab title form.');
		fireEvent.submit(form);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(
			terminalContext.tabModel.getSnapshot().tabs.find((tab) => tab.id === firstTabId)?.title,
		).toBe('system');
		expect(document.activeElement).toBe(titleReturnFocus);
	});

	it('names new and existing Tab groups in the group name dialog', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('tab-group-name'),
		});
		const { tabModel } = projectWorkspaceRuntime.getActiveTerminalContext();
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		fireEvent.contextMenu(screen.getByRole('tab', { name: 'system' }), {
			clientX: 120,
			clientY: 20,
		});
		const tabMenu = await screen.findByRole('menu', { name: 'Tab options' });
		fireEvent.click(within(tabMenu).getByRole('menuitem', { name: 'Add to group' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'New group…' }));
		let dialog = await screen.findByRole('dialog', { name: 'New Tab Group' });
		// The new group is named after the Tab the menu opened on until another name is typed.
		let nameInput = within(dialog).getByLabelText<HTMLInputElement>('New Tab Group');
		expect(nameInput.value).toBe('system');
		expect(within(dialog).getByText('Leave blank for an unnamed group.')).toBeDefined();
		fireEvent.change(nameInput, { target: { value: 'Build' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Create' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		const group = tabModel.getSnapshot().groups[0];
		expect(group).toMatchObject({ color: 'blue', title: 'Build' });
		expect(tabModel.getSnapshot().tabs[0]?.group).toBe(group?.id);

		fireEvent.contextMenu(screen.getByRole('tablist', { name: /^Build,/ }), {
			clientX: 60,
			clientY: 20,
		});
		const groupMenu = await screen.findByRole('menu', { name: 'Group options' });
		fireEvent.click(within(groupMenu).getByRole('menuitem', { name: 'Rename' }));
		dialog = await screen.findByRole('dialog', { name: 'Rename Tab Group' });
		nameInput = within(dialog).getByLabelText<HTMLInputElement>('Rename Tab Group');
		expect(nameInput.value).toBe('Build');
		// Blank leaves the group unnamed rather than being refused.
		fireEvent.change(nameInput, { target: { value: '' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Rename' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(tabModel.getSnapshot().groups[0]?.title).toBe('');
	});

	it('gives the terminal context menu the panel menu composition, Maximize beside another panel', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('workspace-menu'),
		});
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		const openTerminalMenu = async (): Promise<HTMLElement> => {
			const terminalViewport = document.querySelector<HTMLElement>(
				'[data-slot="workbench-terminal-viewport"]',
			);
			if (terminalViewport === null) throw new Error('Expected a terminal Context Menu trigger.');
			const contextMenuEvent = new MouseEvent('contextmenu', {
				bubbles: true,
				cancelable: true,
				clientX: 400,
				clientY: 300,
			});
			fireEvent(terminalViewport, contextMenuEvent);
			expect(contextMenuEvent.defaultPrevented).toBe(true);
			return screen.findByRole('menu');
		};

		const menu = await openTerminalMenu();
		expectActionableMenuItemsToHaveIcons(menu);
		expect(getActionableMenuPresentation(menu)).toEqual([
			{ icon: 'terminal', label: 'New child terminal' },
			{ icon: 'terminal', label: 'Broadcast Input' },
			{ icon: 'panels', label: 'Split right⌘D' },
			{ icon: 'panels', label: 'Split below⇧⌘D' },
			{ icon: 'close', label: 'Close⌘W' },
		]);
		expect(
			within(menu)
				.getByRole('menuitem', { name: /^Close/u })
				.getAttribute('data-variant'),
		).toBe('destructive');
		fireEvent.click(within(menu).getByRole('menuitem', { name: /^Split right/u }));
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		await waitFor(() =>
			expect(document.querySelectorAll('[data-slot="workbench-terminal-viewport"]')).toHaveLength(
				2,
			),
		);

		expect(getActionableMenuPresentation(await openTerminalMenu())).toEqual([
			{ icon: 'terminal', label: 'New child terminal' },
			{ icon: 'terminal', label: 'Broadcast Input' },
			{ icon: 'panels', label: 'Split right⌘D' },
			{ icon: 'panels', label: 'Split below⇧⌘D' },
			{ icon: 'screen-full', label: 'Maximize⇧⌘Enter' },
			{ icon: 'close', label: 'Close⌘W' },
		]);
		fireEvent.keyDown(document, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
	});

	it.each([
		{
			cancel: 'Cancel',
			close: 'Close',
			create: 'Create',
			description:
				'This folder becomes the starting location for the Project and its Primary Workspace.',
			locale: 'en-US' as const,
			newFolder: 'New Folder…',
			pathDescription: 'Enter an absolute path. The browser cannot verify that it exists.',
			pathLabel: 'Folder path',
			pathPlaceholder: '/path/to/project',
			projectAction: 'Create Project',
			selectFolder: 'Select Folder…',
			title: 'Select Project Folder',
		},
		{
			cancel: '取消',
			close: '关闭',
			create: '创建',
			description: '此文件夹将作为项目及其主工作区的起始位置。',
			locale: 'zh-CN' as const,
			newFolder: '新建文件夹…',
			pathDescription: '请输入绝对路径。浏览器无法验证该路径是否存在。',
			pathLabel: '文件夹路径',
			pathPlaceholder: '/path/to/project',
			projectAction: '创建项目',
			selectFolder: '选择文件夹…',
			title: '选择项目文件夹',
		},
	])(
		'creates a Project from a manually entered Folder path in $locale',
		async ({
			cancel,
			close,
			create,
			description,
			locale,
			newFolder,
			pathDescription,
			pathLabel,
			pathPlaceholder,
			projectAction,
			selectFolder,
			title,
		}) => {
			const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
				standaloneCwd: '/system',
				standaloneTerminalTitle: locale === 'en-US' ? 'Terminal 1' : '终端 1',
				terminalPresentation: createTestTerminalPresentationComposition('workspace-locale'),
			});
			render(
				<OrchbeeI18nProvider locale={locale}>
					<TooltipProvider>
						<Workbench
							actionRuntime={new ActionRuntime()}
							projectWorkspaceRuntime={projectWorkspaceRuntime}
							shortcutPlatform='apple'
						/>
					</TooltipProvider>
				</OrchbeeI18nProvider>,
			);

			expect(document.activeElement).toBe(document.body);
			const projectCreateButton = screen.getByRole('button', { name: projectAction });
			expect(projectCreateButton.hasAttribute('data-base-ui-tooltip-trigger')).toBe(true);
			fireEvent.click(projectCreateButton);
			expect(screen.queryByRole('dialog')).toBeNull();
			const menu = await screen.findByRole('menu');
			expect(menu.className).toContain('w-(--ob-ref-size-menu)');
			expect(projectCreateButton.hasAttribute('data-popup-open')).toBe(true);
			const newFolderItem = within(menu).getByRole('menuitem', { name: newFolder });
			expect(newFolderItem.getAttribute('aria-disabled')).toBe('true');
			expectActionableMenuItemsToHaveIcons(menu);
			fireEvent.click(within(menu).getByRole('menuitem', { name: selectFolder }));
			let dialog = await screen.findByRole('dialog');
			expect(dialog.className).toContain('shadow-lg');
			expect(dialog.className).toContain('sm:max-w-(--ob-ref-size-dialog)');
			expect(dialog.className).not.toContain('ring-foreground/10');
			const dialogFooter = dialog.querySelector<HTMLElement>('[data-slot="dialog-footer"]');
			expect(dialogFooter?.className).not.toContain('border-t');
			expect(dialogFooter?.className).not.toContain('bg-muted/50');
			expect(within(dialog).getByRole('heading', { name: title })).toBeDefined();
			expect(within(dialog).getByText(description)).toBeDefined();
			expect(within(dialog).getByText(pathDescription)).toBeDefined();
			expect(within(dialog).getByRole('button', { name: close })).toBeDefined();
			const pathInput = within(dialog).getByLabelText(pathLabel);
			expect(pathInput.getAttribute('placeholder')).toBe(pathPlaceholder);
			expect(pathInput.getAttribute('aria-describedby')).toBe('project-create-path-description');
			expect(pathInput.getAttribute('data-code-like')).toBe('true');
			const cancelButton = within(dialog).getByRole('button', { name: cancel });
			const createButton = within(dialog).getByRole('button', { name: create });
			expect(within(cancelButton).getByText('Esc')).toBeDefined();
			expect(within(createButton).getByText('Enter')).toBeDefined();
			expect(createButton.getAttribute('type')).toBe('submit');
			fireEvent.change(pathInput, { target: { value: '   ' } });
			expect(createButton).toHaveProperty('disabled', true);
			fireEvent.keyDown(document, { key: 'Escape' });
			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
			expect(projectWorkspaceRuntime.getSnapshot().projects).toEqual([]);

			fireEvent.click(screen.getByRole('button', { name: projectAction }));
			fireEvent.click(
				within(await screen.findByRole('menu')).getByRole('menuitem', { name: selectFolder }),
			);
			dialog = await screen.findByRole('dialog');
			fireEvent.change(within(dialog).getByLabelText(pathLabel), {
				target: { value: ' /workspace/orchbee/ ' },
			});
			const form = dialog.querySelector('form');
			expect(form).not.toBeNull();
			if (form === null) throw new Error('Expected the Project creation form to exist.');
			fireEvent.submit(form);

			await waitFor(() => {
				expect(projectWorkspaceRuntime.getSnapshot()).toMatchObject({
					activeLocation: {
						kind: 'workspace',

						workspaceId: 'workspace-1',
					},
					resourceScopes: [
						{
							directories: [{ name: 'orchbee', path: '/workspace/orchbee/' }],
							name: 'orchbee',
						},
					],
				});
			});
			expect(screen.queryByRole('dialog')).toBeNull();
		},
	);

	it.each([
		{
			locale: 'en-US',
			projectAction: 'Create Project',
			newFolder: 'New Folder…',
			name: 'Folder name',
			cancel: 'Cancel',
			create: 'Create',
		},
		{
			locale: 'zh-CN',
			projectAction: '创建项目',
			newFolder: '新建文件夹…',
			name: '文件夹名称',
			cancel: '取消',
			create: '创建',
		},
	] as const)(
		'creates a new Folder only after explicit submission and starts its exact Primary terminal in $locale',
		async ({ locale, projectAction, newFolder, name, cancel, create }) => {
			const runtime = new WorkbenchProjectWorkspaceRuntime({
				standaloneCwd: '/system',
				standaloneTerminalTitle: 'Terminal',
				terminalPresentation: createTestManagedTerminalPresentationComposition('new-folder'),
			});
			const capability: WorkbenchNewProjectFolderCapability = {
				acknowledge: vi.fn<WorkbenchNewProjectFolderCapability['acknowledge']>(),
				confirmProject: vi.fn<WorkbenchNewProjectFolderCapability['confirmProject']>(
					async () => true,
				),
				releaseParent: vi.fn<WorkbenchNewProjectFolderCapability['releaseParent']>(),
				signal: new AbortController().signal,
				selectParent: vi.fn<WorkbenchNewProjectFolderCapability['selectParent']>(async () => ({
					kind: 'parent-selected',
					parent: { path: '/workspace', selectionId: 'parent:1' },
				})),
				create: vi
					.fn<WorkbenchNewProjectFolderCapability['create']>()
					.mockResolvedValueOnce({ code: 'already-exists', kind: 'failed' })
					.mockResolvedValue({
						kind: 'created',
						path: '/workspace/orchbee',
						requestId: 'create:2',
					}),
			};
			render(
				<OrchbeeI18nProvider locale={locale}>
					<TooltipProvider>
						<Workbench
							actionRuntime={new ActionRuntime()}
							projectFolderAcquisition={{
								newFolder: capability,
								selectExistingFolder: async () => ({ kind: 'cancelled' }),
							}}
							projectWorkspaceRuntime={runtime}
							shortcutPlatform='standard'
						/>
					</TooltipProvider>
				</OrchbeeI18nProvider>,
			);
			const open = async () => {
				fireEvent.click(screen.getByRole('button', { name: projectAction }));
				fireEvent.click(
					within(await screen.findByRole('menu')).getByRole('menuitem', { name: newFolder }),
				);
				return screen.findByRole('dialog');
			};
			let dialog = await open();
			expect(within(dialog).getByText('/workspace')).toBeDefined();
			fireEvent.click(within(dialog).getByRole('button', { name: cancel }));
			expect(capability.create).not.toHaveBeenCalled();
			expect(runtime.getSnapshot().projects).toEqual([]);
			dialog = await open();
			fireEvent.change(within(dialog).getByLabelText(name), { target: { value: 'taken' } });
			fireEvent.click(within(dialog).getByRole('button', { name: create }));
			await waitFor(() => expect(within(dialog).getByRole('alert')).toBeDefined());
			expect(runtime.getSnapshot().projects).toEqual([]);
			expect(capability.selectParent).toHaveBeenCalledTimes(2);
			fireEvent.change(within(dialog).getByLabelText(name), { target: { value: 'orchbee' } });
			fireEvent.click(within(dialog).getByRole('button', { name: create }));
			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
			expect(capability.create).toHaveBeenCalledTimes(2);
			expect(capability.acknowledge).toHaveBeenCalledExactlyOnceWith('create:2');
			expect(runtime.getSnapshot()).toMatchObject({
				activeLocation: { kind: 'workspace', workspaceId: 'workspace-1' },
				resourceScopes: [{ directories: [{ path: '/workspace/orchbee' }] }],
				workspaces: [{ id: 'workspace-1' }],
			});
			const first = runtime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot()
				.tabs[0]?.root;
			expect(first).toMatchObject({
				kind: 'panel',
				primaryPresentation: {
					launchCwd: '/workspace/orchbee',
					lifecycle: { kind: 'provisioning' },
				},
			});
		},
	);

	it('uses a native Folder capability for atomic Project creation', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('native-folder'),
		});
		const selectExistingFolder = vi
			.fn<() => Promise<{ readonly kind: 'selected'; readonly path: string }>>()
			.mockResolvedValueOnce({ kind: 'selected', path: '/workspace/orchbee' });
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectFolderAcquisition={{ selectExistingFolder }}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		fireEvent.click(screen.getByRole('button', { name: 'Create Project' }));
		fireEvent.click(
			within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Select Folder…' }),
		);
		await waitFor(() => {
			expect(projectWorkspaceRuntime.getSnapshot().projects[0]).toMatchObject({ name: 'orchbee' });
			expect(projectWorkspaceRuntime.getSnapshot().resourceScopes[0]).toMatchObject({
				directories: [{ path: '/workspace/orchbee' }],
			});
		});
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(selectExistingFolder).toHaveBeenCalledOnce();
	});

	it('shows a localized bounded native Folder failure without Project mutation', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: '终端 1',
			terminalPresentation: createTestTerminalPresentationComposition('native-folder-failure'),
		});
		render(
			<OrchbeeI18nProvider locale='zh-CN'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectFolderAcquisition={{
							selectExistingFolder: async () => ({
								code: 'permission-denied',
								kind: 'failed',
							}),
						}}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		fireEvent.click(screen.getByRole('button', { name: '创建项目' }));
		fireEvent.click(
			within(await screen.findByRole('menu')).getByRole('menuitem', { name: '选择文件夹…' }),
		);
		const failure = await screen.findByRole('alertdialog');
		expect(within(failure).getByText('无法添加文件夹')).toBeDefined();
		expect(within(failure).getByText('Orchbee 无法读取或进入所选文件夹。')).toBeDefined();
		expect(projectWorkspaceRuntime.getSnapshot().projects).toEqual([]);
	});

	it('selects the Workspace created from a current Project Folder', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal',
			terminalPresentation: createTestTerminalPresentationComposition('folder-workspace'),
		});
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-orchbee',
				kind: 'folder',
				name: 'orchbee',
				path: '/workspace/orchbee',
				placement: { kind: 'local' },
				source: 'selected',
			},
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'orchbee',
			terminalTitle: 'Terminal',
			type: 'project-created',
		});
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		fireEvent.click(screen.getByRole('button', { name: 'Project options: orchbee' }));
		const projectMenu = await screen.findByRole('menu');
		fireEvent.click(within(projectMenu).getByRole('menuitem', { name: 'New Workspace' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'orchbee' }));

		await waitFor(() =>
			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',

				workspaceId: 'workspace-2',
			}),
		);
		const workspace = document.querySelector<HTMLElement>('[data-workspace-id="workspace-2"]');
		expect(
			workspace
				?.querySelector('[data-slot="workbench-sidebar-workspace"]')
				?.getAttribute('aria-current'),
		).toBe('page');
	});

	it('composes Project and Workspace row menus as the TUI does and renames through one field', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('workspace-folder'),
		});
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-orchbee',
				kind: 'folder',
				name: 'orchbee',
				path: '/workspace/orchbee',
				placement: { kind: 'local' },
				source: 'selected',
			},
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		const projectOptions = screen.getByRole('button', { name: 'Project options: orchbee' });
		expect(projectOptions.querySelector('[data-icon]')?.getAttribute('data-icon')).toBe('ellipsis');
		const workspaceOptions = screen.getByRole('button', { name: 'Workspace options: Primary' });
		expect(workspaceOptions.querySelector('[data-icon]')?.getAttribute('data-icon')).toBe(
			'ellipsis',
		);
		fireEvent.click(workspaceOptions);
		const workspaceMenu = await screen.findByRole('menu');
		expectActionableMenuItemsToHaveIcons(workspaceMenu);
		const workspaceMenuPresentation = getActionableMenuPresentation(workspaceMenu);
		expect(workspaceMenuPresentation).toEqual([
			{ icon: 'folder', label: 'Move to Project' },
			{ icon: 'edit', label: 'Rename' },
			{ icon: 'close', label: 'Close' },
		]);
		fireEvent.click(within(workspaceMenu).getByRole('menuitem', { name: 'Close' }));
		const deleteWorkspaceTitle = await screen.findByText('Close Workspace?');
		const confirmationDialog = deleteWorkspaceTitle.closest<HTMLElement>(
			'[data-slot="alert-dialog-content"]',
		);
		expect(confirmationDialog).not.toBeNull();
		if (confirmationDialog === null) throw new Error('Expected a modal confirmation Alert Dialog.');
		expect(document.querySelector('[data-slot="alert-dialog-overlay"]')).not.toBeNull();
		expect(document.querySelector('[data-slot="workbench-confirmation-popover"]')).toBeNull();
		const confirmationCancel = within(confirmationDialog).getByRole('button', { name: 'Cancel' });
		const confirmationSubmit = within(confirmationDialog).getByRole('button', {
			name: 'Close',
		});
		expect(within(confirmationCancel).getByText('Esc')).toBeDefined();
		expect(within(confirmationSubmit).queryByText('Enter')).toBeNull();
		await waitFor(() => expect(document.activeElement).toBe(confirmationCancel));
		fireEvent.click(confirmationCancel);
		await waitFor(() =>
			expect(document.querySelector('[data-slot="alert-dialog-content"]')).toBeNull(),
		);
		expect(document.querySelector('[data-slot="alert-dialog-overlay"]')).toBeNull();

		const navigationBeforeContextMenus = projectWorkspaceRuntime.getSnapshot();
		const workspaceRow = workspaceOptions.closest<HTMLElement>(
			'[data-slot="context-menu-trigger"]',
		);
		expect(workspaceRow).not.toBeNull();
		if (workspaceRow === null) throw new Error('Expected the Workspace Context Menu trigger.');
		const workspaceContextMenuEvent = new MouseEvent('contextmenu', {
			bubbles: true,
			cancelable: true,
			clientX: 120,
			clientY: 200,
		});
		fireEvent(workspaceRow, workspaceContextMenuEvent);
		expect(workspaceContextMenuEvent.defaultPrevented).toBe(true);
		const workspaceContextMenu = await screen.findByRole('menu');
		expect(workspaceRow.hasAttribute('data-popup-open')).toBe(true);
		expect(workspaceContextMenu.getAttribute('data-slot')).toBe('context-menu-content');
		expect(getActionableMenuPresentation(workspaceContextMenu)).toEqual(workspaceMenuPresentation);
		fireEvent.click(within(workspaceContextMenu).getByRole('menuitem', { name: 'Rename' }));
		const workspaceRename = await screen.findByRole('dialog', { name: 'Rename Workspace' });
		expect(workspaceRename.getAttribute('data-slot')).toBe('workbench-name-dialog');
		expect(within(workspaceRename).getByRole('button', { name: 'Close' })).toBeDefined();
		// The title names the only field, which has no label or description of its own.
		expect(within(workspaceRename).getByLabelText('Rename Workspace')).toHaveProperty(
			'value',
			'Primary',
		);
		expect(workspaceRename.querySelector('label')).toBeNull();
		const workspaceCancel = within(workspaceRename).getByRole('button', { name: 'Cancel' });
		const workspaceSubmit = within(workspaceRename).getByRole('button', { name: 'Rename' });
		expect(within(workspaceCancel).getByText('Esc')).toBeDefined();
		expect(within(workspaceSubmit).getByText('Enter')).toBeDefined();
		expect(workspaceSubmit.getAttribute('type')).toBe('submit');
		fireEvent.click(workspaceCancel);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(workspaceRow.hasAttribute('data-popup-open')).toBe(false);

		const projectRow = projectOptions.closest<HTMLElement>('[data-slot="context-menu-trigger"]');
		expect(projectRow).not.toBeNull();
		if (projectRow === null) throw new Error('Expected the Project Context Menu trigger.');
		const projectContextMenuEvent = new MouseEvent('contextmenu', {
			bubbles: true,
			cancelable: true,
			clientX: 120,
			clientY: 160,
		});
		fireEvent(projectRow, projectContextMenuEvent);
		expect(projectContextMenuEvent.defaultPrevented).toBe(true);
		const projectContextMenu = await screen.findByRole('menu');
		expect(projectRow.hasAttribute('data-popup-open')).toBe(true);
		expect(projectContextMenu.getAttribute('data-slot')).toBe('context-menu-content');
		const projectContextMenuPresentation = getActionableMenuPresentation(projectContextMenu);
		expect(projectContextMenuPresentation).toEqual([
			{ icon: 'add', label: 'New Workspace' },
			{ icon: 'edit', label: 'Rename' },
			{ icon: 'close', label: 'Dissolve' },
		]);
		fireEvent.keyDown(document, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		expect(projectRow.hasAttribute('data-popup-open')).toBe(false);
		expect(projectWorkspaceRuntime.getSnapshot()).toMatchObject({
			activeLocation: navigationBeforeContextMenus.activeLocation,
			expandedProjectIds: navigationBeforeContextMenus.expandedProjectIds,
		});

		fireEvent.click(projectOptions);
		const projectMenu = await screen.findByRole('menu');
		expectActionableMenuItemsToHaveIcons(projectMenu);
		expect(getActionableMenuPresentation(projectMenu)).toEqual(projectContextMenuPresentation);
		fireEvent.click(within(projectMenu).getByRole('menuitem', { name: 'Rename' }));
		const projectRename = await screen.findByRole('dialog', { name: 'Rename Project' });
		const projectNameInput = within(projectRename).getByLabelText('Rename Project');
		const projectSubmit = within(projectRename).getByRole('button', { name: 'Rename' });
		// A blank or over-long name is refused when submitted, saying why, and the reason then follows
		// the field until the name is fixed.
		fireEvent.change(projectNameInput, { target: { value: '   ' } });
		expect(within(projectRename).queryByRole('alert')).toBeNull();
		fireEvent.click(projectSubmit);
		expect(within(projectRename).getByRole('alert').textContent).toBe('Enter a name.');
		expect(projectNameInput.getAttribute('aria-invalid')).toBe('true');
		expect(document.activeElement).toBe(projectNameInput);
		fireEvent.change(projectNameInput, { target: { value: 'x'.repeat(513) } });
		expect(within(projectRename).getByRole('alert').textContent).toBe(
			'Use at most 512 characters.',
		);
		fireEvent.click(projectSubmit);
		expect(screen.getByRole('dialog')).toBe(projectRename);
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]?.name).toBe('orchbee');
		// The name is kept exactly as typed.
		fireEvent.change(projectNameInput, { target: { value: ' Orchbee renamed' } });
		expect(within(projectRename).queryByRole('alert')).toBeNull();
		fireEvent.click(projectSubmit);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]?.name).toBe(' Orchbee renamed');

		const beforeDissolve = projectWorkspaceRuntime.getSnapshot();
		fireEvent.click(screen.getByRole('button', { name: 'Project options: Orchbee renamed' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Dissolve' }));
		await waitFor(() => expect(projectWorkspaceRuntime.getSnapshot().projects).toEqual([]));
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toBe(
			beforeDissolve.activeLocation,
		);
		expect(projectWorkspaceRuntime.getSnapshot().workspaces).toEqual(
			beforeDissolve.workspaces.map((workspace) => ({ ...workspace, projectId: null })),
		);
	}, 10_000);

	it('previews Project reorder compactly, commits on release, and restores expansion on completion or cancellation', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('project-reorder'),
		});
		for (const [index, name] of ['Alpha', 'Beta', 'Gamma'].entries()) {
			projectWorkspaceRuntime.transition({
				directory: {
					available: true,
					git: null,
					id: `directory-${index + 1}`,
					kind: 'folder',
					name,
					path: `/workspace/${name.toLowerCase()}`,
					placement: { kind: 'local' },
					source: 'selected',
				},
				launchConfigurations: [],
				primaryWorkspaceLaunchConfigurationId: null,
				primaryWorkspaceName: `${name} Workspace`,
				projectName: name,
				terminalTitle: 'Terminal 1',
				type: 'project-created',
			});
		}
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		const projectItems = () =>
			Array.from(document.querySelectorAll<HTMLElement>('[data-project-id]'));
		for (const item of projectItems()) {
			const row = item.querySelector<HTMLElement>('[data-project-reorder-row]');
			expect(row).not.toBeNull();
			if (row === null) continue;
			row.getBoundingClientRect = () => {
				const index = projectItems().indexOf(item);
				return projectRowRectangle(index * 40);
			};
		}
		const gammaItem = document.querySelector<HTMLElement>('[data-project-id="project-3"]');
		const gammaRow = gammaItem?.querySelector<HTMLElement>('[data-project-reorder-row]');
		const gammaButton = screen.getByRole('button', { name: 'Collapse project: Gamma' });
		expect(gammaItem).not.toBeNull();
		expect(gammaRow).toBeDefined();
		if (gammaItem === null || gammaRow === null || gammaRow === undefined) {
			throw new Error('Expected the Gamma Project row.');
		}

		fireEvent.pointerDown(gammaRow, {
			button: 0,
			clientX: 10,
			clientY: 100,
			pointerId: 7,
			pointerType: 'mouse',
		});
		fireEvent.pointerMove(document, {
			clientX: 14,
			clientY: 50,
			pointerId: 7,
			pointerType: 'mouse',
		});

		await waitFor(() => expect(gammaItem.getAttribute('data-dragging')).toBe('true'));
		expect(
			projectItems().every((item) => item.getAttribute('data-project-reorder-active') === 'true'),
		).toBe(true);
		expect(gammaButton.getAttribute('aria-expanded')).toBe('false');
		expect(gammaItem.querySelector('[data-slot="workbench-sidebar-workspace"]')).toBeNull();
		expect(
			document
				.querySelector<HTMLElement>('[data-project-id="project-1"]')
				?.getAttribute('data-project-reorder-displacement'),
		).toBeNull();
		expect(
			document
				.querySelector<HTMLElement>('[data-project-id="project-2"]')
				?.getAttribute('data-project-reorder-displacement'),
		).toBe('down');

		fireEvent.pointerMove(document, {
			clientX: 14,
			clientY: 1,
			pointerId: 7,
			pointerType: 'mouse',
		});
		expect(projectItems().map((item) => item.dataset['projectId'])).toEqual([
			'project-1',
			'project-2',
			'project-3',
		]);
		expect(
			document
				.querySelector<HTMLElement>('[data-project-id="project-1"]')
				?.getAttribute('data-project-reorder-displacement'),
		).toBe('down');
		expect(
			document
				.querySelector<HTMLElement>('[data-project-id="project-2"]')
				?.getAttribute('data-project-reorder-displacement'),
		).toBe('down');
		expect(
			gammaRow.style.getPropertyValue('--ob-comp-workbench-navigation-reorder-translate-y'),
		).toBe('-99px');
		expect(projectWorkspaceRuntime.getSnapshot().projects.map((project) => project.id)).toEqual([
			'project-1',
			'project-2',
			'project-3',
		]);
		expect(projectWorkspaceRuntime.getSnapshot().expandedProjectIds).toContain('project-3');

		fireEvent.pointerUp(document, {
			clientX: 14,
			clientY: 21,
			pointerId: 7,
			pointerType: 'mouse',
		});
		expect(
			projectItems().every((item) => item.getAttribute('data-project-reorder-active') === null),
		).toBe(true);
		fireEvent.click(gammaButton);
		await waitFor(() => {
			expect(projectWorkspaceRuntime.getSnapshot().projects.map((project) => project.id)).toEqual([
				'project-3',
				'project-1',
				'project-2',
			]);
			expect(gammaItem.getAttribute('data-dragging')).toBeNull();
			expect(
				projectItems().every((item) => item.getAttribute('data-project-reorder-active') === null),
			).toBe(true);
			expect(gammaItem.querySelector('[data-slot="workbench-sidebar-workspace"]')).not.toBeNull();
		});
		expect(gammaButton.getAttribute('aria-expanded')).toBe('true');
		expect(projectWorkspaceRuntime.getSnapshot().expandedProjectIds).toContain('project-3');

		const alphaItem = document.querySelector<HTMLElement>('[data-project-id="project-1"]');
		const alphaRow = alphaItem?.querySelector<HTMLElement>('[data-project-reorder-row]');
		expect(alphaItem).not.toBeNull();
		expect(alphaRow).toBeDefined();
		if (alphaItem === null || alphaRow === null || alphaRow === undefined) {
			throw new Error('Expected the Alpha Project row.');
		}
		fireEvent.pointerDown(alphaRow, {
			button: 0,
			clientX: 10,
			clientY: 60,
			pointerId: 8,
			pointerType: 'pen',
		});
		fireEvent.pointerMove(document, {
			clientX: 10,
			clientY: 200,
			pointerId: 8,
			pointerType: 'pen',
		});
		await waitFor(() => expect(alphaItem.getAttribute('data-dragging')).toBe('true'));
		fireEvent.keyDown(document, { key: 'Escape' });

		await waitFor(() => {
			expect(projectItems().map((item) => item.dataset['projectId'])).toEqual([
				'project-3',
				'project-1',
				'project-2',
			]);
			expect(alphaItem.getAttribute('data-dragging')).toBeNull();
			expect(alphaItem.querySelector('[data-slot="workbench-sidebar-workspace"]')).not.toBeNull();
		});
		expect(
			screen.getByRole('button', { name: 'Collapse project: Alpha' }).getAttribute('aria-expanded'),
		).toBe('true');
	});

	it('reorders Workspace siblings with scoped squeeze motion and preserves activation on release or cancellation', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('workspace-reorder'),
		});
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-1',
				kind: 'folder',
				name: 'Orchbee',
				path: '/workspace/orchbee',
				placement: { kind: 'local' },
				source: 'selected',
			},
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'Orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		for (const [name, terminalTitle] of [
			['Feature', 'Terminal 2'],
			['Review', 'Terminal 3'],
		] as const) {
			projectWorkspaceRuntime.transition({
				directoryId: 'directory-1',
				launchConfigurationId: null,
				name,
				resourceScopeId: 'resource-scope-1',
				projectId: 'project-1',
				terminalTitle,
				type: 'workspace-created',
			});
		}
		const activeLocation = projectWorkspaceRuntime.getSnapshot().activeLocation;
		const expandedProjectIds = projectWorkspaceRuntime.getSnapshot().expandedProjectIds;
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		const workspaceItems = () =>
			Array.from(
				document.querySelectorAll<HTMLElement>('[data-project-id="project-1"] [data-workspace-id]'),
			);
		for (const item of workspaceItems()) {
			const row = item.querySelector<HTMLElement>('[data-navigation-reorder-row]');
			expect(row).not.toBeNull();
			if (row === null) continue;
			row.getBoundingClientRect = () => {
				const index = workspaceItems().indexOf(item);
				return projectRowRectangle(100 + index * 40);
			};
		}
		const reviewItem = document.querySelector<HTMLElement>('[data-workspace-id="workspace-3"]');
		const reviewRow = reviewItem?.querySelector<HTMLElement>('[data-navigation-reorder-row]');
		expect(reviewItem).not.toBeNull();
		expect(reviewRow).toBeDefined();
		if (reviewItem === null || reviewRow === null || reviewRow === undefined) {
			throw new Error('Expected the Review Workspace row.');
		}

		fireEvent.pointerDown(reviewRow, {
			button: 0,
			clientX: 20,
			clientY: 200,
			pointerId: 11,
			pointerType: 'mouse',
		});
		fireEvent.pointerMove(document, {
			clientX: 20,
			clientY: 119,
			pointerId: 11,
			pointerType: 'mouse',
		});

		await waitFor(() => expect(reviewItem.getAttribute('data-dragging')).toBe('true'));
		expect(
			workspaceItems().every(
				(item) => item.getAttribute('data-workspace-reorder-active') === 'true',
			),
		).toBe(true);
		expect(
			document
				.querySelector<HTMLElement>('[data-workspace-id="workspace-1"]')
				?.getAttribute('data-workspace-reorder-displacement'),
		).toBe('down');
		expect(
			document
				.querySelector<HTMLElement>('[data-workspace-id="workspace-2"]')
				?.getAttribute('data-workspace-reorder-displacement'),
		).toBe('down');
		expect(
			reviewRow.style.getPropertyValue('--ob-comp-workbench-navigation-reorder-translate-y'),
		).toBe('-81px');
		expect(
			projectWorkspaceRuntime.getSnapshot().workspaces.map((workspace) => workspace.id),
		).toEqual(['workspace-1', 'workspace-2', 'workspace-3']);

		fireEvent.pointerUp(document, {
			clientX: 20,
			clientY: 119,
			pointerId: 11,
			pointerType: 'mouse',
		});
		expect(
			workspaceItems().every((item) => item.getAttribute('data-workspace-reorder-active') === null),
		).toBe(true);
		await waitFor(() => {
			expect(
				projectWorkspaceRuntime.getSnapshot().workspaces.map((workspace) => workspace.id),
			).toEqual(['workspace-3', 'workspace-1', 'workspace-2']);
			expect(workspaceItems().map((item) => item.dataset['workspaceId'])).toEqual([
				'workspace-3',
				'workspace-1',
				'workspace-2',
			]);
			expect(reviewItem.getAttribute('data-dragging')).toBeNull();
		});
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toBe(activeLocation);
		expect(projectWorkspaceRuntime.getSnapshot().expandedProjectIds).toBe(expandedProjectIds);

		const primaryItem = document.querySelector<HTMLElement>('[data-workspace-id="workspace-1"]');
		const primaryRow = primaryItem?.querySelector<HTMLElement>('[data-navigation-reorder-row]');
		const primaryButton = screen.getByRole('button', { name: 'Primary' });
		expect(primaryItem).not.toBeNull();
		expect(primaryRow).toBeDefined();
		if (primaryItem === null || primaryRow === null || primaryRow === undefined) {
			throw new Error('Expected the Primary Workspace row.');
		}
		fireEvent.pointerDown(primaryRow, {
			button: 0,
			clientX: 20,
			clientY: 140,
			pointerId: 12,
			pointerType: 'pen',
		});
		fireEvent.pointerMove(document, {
			clientX: 20,
			clientY: 260,
			pointerId: 12,
			pointerType: 'pen',
		});
		await waitFor(() => expect(primaryItem.getAttribute('data-dragging')).toBe('true'));
		fireEvent.keyDown(document, { key: 'Escape' });
		fireEvent.click(primaryButton);

		await waitFor(() => {
			expect(
				projectWorkspaceRuntime.getSnapshot().workspaces.map((workspace) => workspace.id),
			).toEqual(['workspace-3', 'workspace-1', 'workspace-2']);
			expect(primaryItem.getAttribute('data-dragging')).toBeNull();
		});
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toBe(activeLocation);
		expect(
			screen
				.getByRole('button', { name: 'Collapse project: Orchbee' })
				.getAttribute('aria-expanded'),
		).toBe('true');
	});
});

function projectRowRectangle(top: number): DOMRect {
	return {
		bottom: top + 40,
		height: 40,
		left: 0,
		right: 240,
		top,
		width: 240,
		x: 0,
		y: top,
		toJSON: () => ({}),
	};
}
