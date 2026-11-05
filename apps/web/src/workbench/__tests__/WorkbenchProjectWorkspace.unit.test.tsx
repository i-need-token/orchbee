// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useEffect, useLayoutEffect, useRef, type ComponentProps, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixtureContextId,
	fixtureDirectoryId,
	fixtureId,
	fixturePanelId,
	fixturePresentationId,
	fixtureProjectId,
	fixtureTabId,
	fixtureWorkspaceId,
	oneWorkspaceSnapshot,
	publishSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotPanel,
	snapshotProject,
	snapshotSplit,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
	type FixtureRecord,
} from '../../__tests__/server-workbench-snapshot';
import { Workbench } from '../Workbench';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAgentSnapshot } from '../types/workbench-agent-observation';
import type { WorkbenchAuthority } from '../types/workbench-authority';
import type { WorkbenchNavigationPort } from '../types/workbench-host-actions';
import type {
	WorkbenchNewProjectFolderCapability,
	WorkbenchProjectCreationReceipt,
} from '../types/workbench-project-folder-acquisition';
import type { WorkbenchTerminalSurfaceProps } from '../types/workbench-terminal-surface';
import { findActiveTerminalFocusTarget } from '../utils/findActiveTerminalFocusTarget';
import { inertWorkbenchAuthority } from '../../__tests__/inertWorkbenchAuthority';

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
	return <textarea data-test-terminal-input={presentation.presentationId} ref={inputReference} />;
}

function terminalInput(sequence: number): HTMLElement | null {
	return document.querySelector<HTMLElement>(
		`[data-test-terminal-input="${fixturePresentationId(sequence)}"]`,
	);
}

const ORCHBEE_DIRECTORY: FixtureRecord = { ...snapshotDirectory(1), path: '/workspace/orchbee' };
const ORCHBEE = snapshotProject(1, 'orchbee');

interface WorkspaceFixture {
	readonly label: string;
	readonly projectId?: string | null | undefined;
	readonly sequence: number;
	readonly tabs?: readonly FixtureRecord[] | undefined;
}

/**
 * Workspaces in Directory 1 at `/workspace/orchbee`, each with its own terminal context and, unless
 * it says otherwise, in Project 1. `displayOrder` holds a `projectOrder` or `workspaceOrder`.
 */
function navigationSnapshot(
	revision: number,
	projects: readonly FixtureRecord[],
	workspaces: readonly WorkspaceFixture[],
	displayOrder: FixtureRecord = {},
): WorkbenchSnapshot {
	return workbenchSnapshot(
		revision,
		{
			...displayOrder,
			directories: [ORCHBEE_DIRECTORY],
			projects,
			workspaces: workspaces.map(({ label, projectId = fixtureProjectId(1), sequence }) =>
				snapshotWorkspace(sequence, {
					label,
					primaryDirectoryId: fixtureDirectoryId(1),
					projectId,
				}),
			),
		},
		workspaces.map(({ sequence, tabs = [snapshotTab(sequence)] }) =>
			snapshotContext(sequence, tabs),
		),
	);
}

/** Project `orchbee` with one Workspace per label. */
function orchbeeSnapshot(revision: number, labels: readonly string[] = ['Primary']) {
	return navigationSnapshot(
		revision,
		[ORCHBEE],
		labels.map((label, index) => ({ label, sequence: index + 1 })),
	);
}

function emptySnapshot(): WorkbenchSnapshot {
	return workbenchSnapshot(1, { workspaces: [] }, []);
}

/** What the authority reports once the runtime holds the Workspace it created. */
function receiptFor(
	runtime: WorkbenchProjectWorkspaceRuntime,
	sequence: number,
): WorkbenchProjectCreationReceipt {
	const workspace = runtime
		.getSnapshot()
		.workspaces.find((candidate) => candidate.id === fixtureWorkspaceId(sequence));
	if (workspace?.directory === undefined || workspace.directory === null) {
		throw new Error('Expected the published Workspace and its Directory.');
	}
	return {
		directoryId: workspace.directory.id,
		path: '/workspace/orchbee',
		presentationId: fixturePresentationId(sequence),
		projectId: workspace.projectId,
		terminalContextId: workspace.terminalContextId,
		workspaceId: workspace.id,
	};
}

/** Publishes `snapshot`, then reports Workspace `sequence` created, as the Server does. */
function publishingCreate(
	runtime: WorkbenchProjectWorkspaceRuntime,
	snapshot: WorkbenchSnapshot,
	sequence: number,
) {
	return vi.fn<WorkbenchAuthority['create']>(async () => {
		publishSnapshot(runtime, snapshot);
		return { kind: 'completed', value: receiptFor(runtime, sequence) };
	});
}

function completingChange() {
	return vi.fn<WorkbenchAuthority['change']>(async () => ({ kind: 'completed' }));
}

type WorkbenchTestProps = Pick<
	ComponentProps<typeof Workbench>,
	| 'agentObservationSource'
	| 'navigation'
	| 'projectFolderAcquisition'
	| 'projectWorkspaceRuntime'
	| 'terminalSurface'
> & {
	readonly authority?: WorkbenchAuthority | undefined;
	readonly locale?: 'en-US' | 'zh-CN' | undefined;
	readonly shortcutPlatform?: 'apple' | 'standard' | undefined;
};

function renderWorkbench({
	agentObservationSource,
	authority = inertWorkbenchAuthority(),
	locale = 'en-US',
	navigation,
	projectFolderAcquisition,
	projectWorkspaceRuntime,
	shortcutPlatform = 'apple',
	terminalSurface,
}: WorkbenchTestProps) {
	return render(
		<OrchbeeI18nProvider locale={locale}>
			<TooltipProvider>
				<Workbench
					actionRuntime={new ActionRuntime()}
					agentObservationSource={agentObservationSource}
					authority={authority}
					navigation={navigation}
					projectFolderAcquisition={projectFolderAcquisition}
					projectWorkspaceRuntime={projectWorkspaceRuntime}
					shortcutPlatform={shortcutPlatform}
					terminalSurface={terminalSurface}
				/>
			</TooltipProvider>
		</OrchbeeI18nProvider>,
	);
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
		const projectWorkspaceRuntime = createSnapshotRuntime(
			orchbeeSnapshot(1, ['Primary', 'Primary']),
		);
		projectWorkspaceRuntime.transition({
			type: 'workspace-activated',
			workspaceId: fixtureWorkspaceId(2),
		});
		const change = completingChange();
		const { container } = renderWorkbench({
			authority: inertWorkbenchAuthority({ change }),
			projectWorkspaceRuntime,
			shortcutPlatform: 'standard',
		});
		const workspace = container.querySelector<HTMLElement>(
			`[data-workspace-id="${fixtureWorkspaceId(1)}"]`,
		);
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
		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			name: 'First only',
			type: 'workspace-updated',
			workspaceId: fixtureWorkspaceId(1),
		});
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toBe(snapshot.activeLocation);
	});

	it('leaves composing, repeated, modified and terminal-owned menu keys unconsumed', () => {
		renderWorkbench({
			projectWorkspaceRuntime: createSnapshotRuntime(orchbeeSnapshot(1)),
			shortcutPlatform: 'standard',
			terminalSurface: FocusTestTerminalSurface,
		});
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
		const terminal = terminalInput(1);
		if (terminal === null) throw new Error('Expected the Workspace terminal input.');
		for (const chord of [{ code: 'F10', shiftKey: true }, { code: 'ContextMenu' }]) {
			const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...chord });
			fireEvent(terminal, event);
			expect(event.defaultPrevented).toBe(false);
			expect(screen.queryByRole('menu')).toBeNull();
		}
	});

	it('returns context-menu focus to the row and respects outside, dialog and removal handoffs', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(orchbeeSnapshot(1));
		const close = vi.fn<WorkbenchAuthority['close']>(async () => ({ kind: 'completed' }));
		renderWorkbench({
			authority: inertWorkbenchAuthority({ close }),
			projectWorkspaceRuntime,
			shortcutPlatform: 'standard',
		});
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
			publishSnapshot(projectWorkspaceRuntime, navigationSnapshot(2, [ORCHBEE], []));
		});
		act(() => outside.focus());
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		const afterRemoval = projectWorkspaceRuntime.getSnapshot();
		fireEvent.click(staleCommand);
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(afterRemoval);
		expect(document.activeElement).toBe(outside);
		expect(options.isConnected).toBe(false);
		expect(close).not.toHaveBeenCalled();
	});

	it('creates independently, groups through the Workspace menu, and dissolves through the authority', async () => {
		const runtime = createSnapshotRuntime(emptySnapshot());
		const review = snapshotProject(1, 'Review');
		const workspaceIn = (revision: number, projectId: string | null) =>
			navigationSnapshot(revision, projectId === null ? [] : [review], [
				{ label: 'Workspace', projectId, sequence: 1 },
			]);
		const create = publishingCreate(runtime, workspaceIn(2, null), 1);
		const change = completingChange();
		renderWorkbench({
			authority: inertWorkbenchAuthority({ change, create }),
			projectWorkspaceRuntime: runtime,
			shortcutPlatform: 'standard',
		});
		const newWorkspace = screen.getByRole('button', { name: 'New workspace' });
		const fixedActions = newWorkspace.closest('[data-slot="workbench-sidebar-actions"]');
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
		fireEvent.click(newWorkspace);
		await waitFor(() => expect(runtime.getSnapshot().workspaces).toHaveLength(1));
		fireEvent.click(screen.getByRole('button', { name: 'Collapse Recents' }));
		await waitFor(() => expect(screen.queryByRole('button', { name: 'Workspace' })).toBeNull());
		fireEvent.click(screen.getByRole('button', { name: 'Expand Recents' }));
		await screen.findByRole('button', { name: 'Workspace' });
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(create).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			focusedPanel: null,
			projectId: null,
			type: 'workspace-created-default',
		});
		const workspaceId = fixtureWorkspaceId(1);
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
		await waitFor(() =>
			expect(change).toHaveBeenLastCalledWith(expect.anything(), {
				name: 'Review',
				type: 'workspace-project-created',
				workspaceId,
			}),
		);
		act(() => publishSnapshot(runtime, workspaceIn(3, fixtureProjectId(1))));
		await openMove();
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Remove from Project' }));
		await waitFor(() =>
			expect(change).toHaveBeenLastCalledWith(expect.anything(), {
				projectId: null,
				type: 'workspace-project-set',
				workspaceId,
			}),
		);
		act(() =>
			publishSnapshot(
				runtime,
				navigationSnapshot(4, [review], [{ label: 'Workspace', projectId: null, sequence: 1 }]),
			),
		);
		await openMove();
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Review' }));
		await waitFor(() =>
			expect(change).toHaveBeenLastCalledWith(expect.anything(), {
				projectId: fixtureProjectId(1),
				type: 'workspace-project-set',
				workspaceId,
			}),
		);
		act(() => publishSnapshot(runtime, workspaceIn(5, fixtureProjectId(1))));
		fireEvent.click(screen.getByRole('button', { name: 'Project options: Review' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Dissolve' }));
		await waitFor(() =>
			expect(change).toHaveBeenLastCalledWith(expect.anything(), {
				projectId: fixtureProjectId(1),
				type: 'project-dissolved',
			}),
		);
		expect(change).toHaveBeenCalledTimes(4);
		expect(screen.queryByRole('alertdialog')).toBeNull();
	});

	it('lists observed Workspace Agents until folded, keeps the fold, and opens the exact terminal', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(
			navigationSnapshot(
				1,
				[ORCHBEE],
				[
					{ label: 'Primary', sequence: 1, tabs: [snapshotTab(1), snapshotTab(2)] },
					{ label: 'Elsewhere', projectId: null, sequence: 3, tabs: [snapshotTab(3)] },
				],
			),
		);
		const workspace = projectWorkspaceRuntime.getSnapshot().workspaces[0];
		if (workspace === undefined) throw new Error('Expected the primary Workspace.');
		const context = projectWorkspaceRuntime.getTerminalContext(workspace.terminalContextId);
		if (context === null) throw new Error('Expected the Workspace terminal context.');
		const presentationId = fixturePresentationId(2);
		const targetTabTitle = 'Tab 2';
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
		projectWorkspaceRuntime.transition({
			type: 'workspace-activated',
			workspaceId: fixtureWorkspaceId(3),
		});
		renderWorkbench({
			agentObservationSource: agentSource,
			projectWorkspaceRuntime,
			terminalSurface: FocusTestTerminalSurface,
		});

		// As in the TUI, a Workspace lists its Agents until this Client folds them away.
		const workspaceButton = screen.getByRole('button', {
			name: 'Primary. 1 live Agent. Codex: Ready',
		});
		const workspaceRow = workspaceButton.closest('[data-workspace-id]');
		const agentName = `Codex, Ready, ${targetTabTitle} · Panel 1 · Primary terminal`;
		const agentButton = screen.getByRole('button', { name: agentName });
		expect(agentButton.closest('li')?.textContent).toContain('Codex');

		// Folded, the row's second line lists the states, and the fold outlasts a folded Project and a
		// hidden sidebar.
		fireEvent.click(screen.getByRole('button', { name: 'Hide 1 Agent in Primary' }));
		expect(screen.queryByRole('button', { name: agentName })).toBeNull();
		const states = workspaceRow?.querySelector('[data-slot="workbench-workspace-agent-status"]');
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
			expect(context.tabModel.getSnapshot().activeTabId).toBe(fixtureTabId(2));
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
			expect(
				screen.getByRole('button', { name: 'Primary' }).closest('[data-workspace-id]')?.textContent,
			).toContain('Primary');
		});
	});

	it('focuses the Workspace terminal only after pointer-click navigation is published', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(
			orchbeeSnapshot(1, ['Primary', 'Secondary']),
		);
		projectWorkspaceRuntime.transition({
			type: 'workspace-activated',
			workspaceId: fixtureWorkspaceId(2),
		});
		const navigate = vi.fn<WorkbenchNavigationPort['navigate']>((context) => {
			context.markCommitted();
			return { kind: 'completed' };
		});
		renderWorkbench({
			navigation: { navigate, selectTab: () => ({ kind: 'unchanged' }) },
			projectWorkspaceRuntime,
			terminalSurface: FocusTestTerminalSurface,
		});

		const workspaceButton = screen.getByRole('button', { name: 'Primary' });
		act(() => workspaceButton.focus());
		fireEvent.click(workspaceButton, { detail: 1 });
		await waitFor(() =>
			expect(navigate).toHaveBeenCalledWith(expect.anything(), {
				kind: 'workspace',
				workspaceId: fixtureWorkspaceId(1),
			}),
		);
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: fixtureWorkspaceId(2),
		});
		expect(document.activeElement).toBe(workspaceButton);

		act(() => {
			projectWorkspaceRuntime.transition({
				type: 'workspace-activated',
				workspaceId: fixtureWorkspaceId(1),
			});
		});
		await waitFor(() =>
			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',
				workspaceId: fixtureWorkspaceId(1),
			}),
		);
		await waitFor(() => expect(document.activeElement).toBe(terminalInput(1)));
	});

	it('steps the sidebar aside in a narrow window and closes its overlay on Esc, a click outside, or a chosen row', async () => {
		vi.stubGlobal('matchMedia', (query: string) => ({
			addEventListener: () => {},
			matches: query === '(width < 731px)',
			removeEventListener: () => {},
		}));
		const projectWorkspaceRuntime = createSnapshotRuntime(
			orchbeeSnapshot(1, ['Primary', 'Secondary']),
		);
		projectWorkspaceRuntime.transition({
			type: 'workspace-activated',
			workspaceId: fixtureWorkspaceId(2),
		});
		renderWorkbench({ projectWorkspaceRuntime, terminalSurface: FocusTestTerminalSurface });
		const overlay = () => document.querySelector('[data-slot="workbench-sidebar-overlay"]');
		const openOverlay = async () => {
			fireEvent.click(screen.getByRole('button', { name: 'Show Sidebar' }));
			await waitFor(() => expect(overlay()).not.toBeNull());
		};

		// Hidden on its own; the toggle opens it over the content, and it takes the keyboard.
		expect(document.querySelector('[data-slot="workbench-left-panel"]')).toBeNull();
		await openOverlay();
		await waitFor(() =>
			expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Secondary' })),
		);

		const search = document.querySelector('[data-slot="workbench-sidebar-search"]');
		if (search === null) throw new Error('Expected the Search row.');
		fireEvent.keyDown(search, { key: 'Escape' });
		await waitFor(() => expect(overlay()).toBeNull());

		await openOverlay();
		const scrim = document.querySelector('[data-slot="workbench-sidebar-scrim"]');
		if (scrim === null) throw new Error('Expected the overlay scrim.');
		fireEvent.pointerDown(scrim);
		await waitFor(() => expect(overlay()).toBeNull());

		await openOverlay();
		fireEvent.click(screen.getByRole('button', { name: 'Primary' }), { detail: 1 });
		await waitFor(() => expect(overlay()).toBeNull());
		await waitFor(() =>
			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',
				workspaceId: fixtureWorkspaceId(1),
			}),
		);
	});

	it('keeps the list one tab stop, moves with the arrows, and opens with Enter', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(
			orchbeeSnapshot(1, ['Primary', 'Secondary']),
		);
		projectWorkspaceRuntime.transition({
			type: 'workspace-activated',
			workspaceId: fixtureWorkspaceId(2),
		});
		renderWorkbench({ projectWorkspaceRuntime, terminalSurface: FocusTestTerminalSurface });
		const focused = () => {
			const element = document.activeElement;
			if (!(element instanceof HTMLElement)) throw new Error('Expected a focused element.');
			return element;
		};

		// Tab enters at the current Workspace; no other row or row tool is a tab stop.
		const secondary = screen.getByRole('button', { name: 'Secondary' });
		const primary = screen.getByRole('button', { name: 'Primary' });
		expect(secondary.tabIndex).toBe(0);
		expect(primary.tabIndex).toBe(-1);
		expect(screen.getByRole('button', { name: 'Workspace options: Primary' }).tabIndex).toBe(-1);

		act(() => secondary.focus());
		fireEvent.keyDown(secondary, { key: 'ArrowUp' });
		expect(focused()).toBe(primary);
		expect(primary.tabIndex).toBe(0);
		// ← moves to the heading, then folds it; → unfolds it again.
		fireEvent.keyDown(primary, { key: 'ArrowLeft' });
		expect(focused()).toBe(screen.getByRole('button', { name: 'Collapse project: orchbee' }));
		fireEvent.keyDown(focused(), { key: 'ArrowLeft' });
		expect(focused()).toBe(screen.getByRole('button', { name: 'Expand project: orchbee' }));
		expect(screen.queryByRole('button', { name: 'Primary' })).toBeNull();
		fireEvent.keyDown(focused(), { key: 'ArrowRight' });
		fireEvent.keyDown(focused(), { key: 'ArrowDown' });
		expect(focused()).toBe(screen.getByRole('button', { name: 'Primary' }));

		// Enter opens the Workspace and returns to its terminal.
		fireEvent.keyDown(focused(), { key: 'Enter' });
		await waitFor(() =>
			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',
				workspaceId: fixtureWorkspaceId(1),
			}),
		);
		await waitFor(() => expect(document.activeElement).toBe(terminalInput(1)));
	});

	it('adapts New Tab through exact terminal creation and selects only its returned Tab', async () => {
		const projects = [snapshotProject(1, 'alpha'), snapshotProject(2, 'beta')];
		const betaWithTabs = (revision: number, tabs: readonly FixtureRecord[]) =>
			navigationSnapshot(revision, projects, [
				{ label: 'alpha workspace', sequence: 1 },
				{ label: 'beta workspace', projectId: fixtureProjectId(2), sequence: 2, tabs },
			]);
		const projectWorkspaceRuntime = createSnapshotRuntime(betaWithTabs(1, [snapshotTab(2)]));
		projectWorkspaceRuntime.transition({
			type: 'workspace-activated',
			workspaceId: fixtureWorkspaceId(2),
		});
		const tabsOf = (sequence: number) =>
			projectWorkspaceRuntime
				.getTerminalContext(fixtureContextId(sequence))
				?.tabModel.getSnapshot();
		const alphaSelection = tabsOf(1)?.activeTabId;
		if (alphaSelection === undefined || alphaSelection === null) {
			throw new Error('Expected the alpha Tab selection.');
		}
		const createTab = vi.fn<WorkbenchAuthority['createTab']>(async () => {
			publishSnapshot(projectWorkspaceRuntime, betaWithTabs(2, [snapshotTab(2), snapshotTab(3)]));
			return {
				kind: 'completed',
				value: {
					tabId: fixtureTabId(3),
					terminalContextId: fixtureContextId(2),
					terminalStarted: true,
				},
			};
		});
		renderWorkbench({
			authority: inertWorkbenchAuthority({ createTab }),
			projectWorkspaceRuntime,
		});

		fireEvent.click(screen.getByRole('button', { name: /new tab/iu }));
		await waitFor(() => expect(tabsOf(2)?.tabs).toHaveLength(2));

		expect(createTab).toHaveBeenCalledExactlyOnceWith(
			expect.anything(),
			expect.objectContaining({ terminalContextId: fixtureContextId(2) }),
		);
		expect(tabsOf(1)).toMatchObject({
			activeTabId: alphaSelection,
			tabs: [{ id: alphaSelection }],
		});
		expect(tabsOf(2)?.activeTabId).toBe(fixtureTabId(3));
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: fixtureWorkspaceId(2),
		});
	});

	it('renames the captured Tab from the shortcut and context menu', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2)]),
		);
		const terminalContext = activeTerminalContext(projectWorkspaceRuntime);
		const firstTabId = fixtureTabId(1);
		terminalContext.tabModel.selectTab(firstTabId);
		const change = completingChange();
		renderWorkbench({ authority: inertWorkbenchAuthority({ change }), projectWorkspaceRuntime });
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
		// The title names the only field.
		let titleInput = within(dialog).getByLabelText<HTMLInputElement>('Rename Tab');
		expect(titleInput.getAttribute('aria-required')).toBe('true');
		const titleCancel = within(dialog).getByRole('button', { name: 'Cancel' });
		const titleSave = within(dialog).getByRole('button', { name: 'Rename' });
		expect(titleInput.value).toBe('Tab 1');
		await waitFor(() => expect(document.activeElement).toBe(titleInput));
		expect(titleInput.selectionStart).toBe(0);
		expect(titleInput.selectionEnd).toBe(titleInput.value.length);
		expect(within(titleCancel).getByText('Esc')).toBeDefined();
		expect(within(titleSave).getByText('Enter')).toBeDefined();
		terminalContext.tabModel.selectTab(fixtureTabId(2));
		fireEvent.change(titleInput, { target: { value: '  Focus work  ' } });
		let form = dialog.querySelector('form');
		expect(form).not.toBeNull();
		if (form === null) throw new Error('Expected the Tab title form.');
		fireEvent.submit(form);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			label: 'Focus work',
			tabId: firstTabId,
			terminalContextId: fixtureContextId(1),
			type: 'tab-renamed',
		});
		act(() =>
			publishSnapshot(
				projectWorkspaceRuntime,
				oneWorkspaceSnapshot(2, [
					snapshotTab(1, undefined, { label: 'Focus work' }),
					snapshotTab(2),
				]),
			),
		);

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
		// A shared name cannot return to following the terminal, so a blank one is refused.
		fireEvent.change(titleInput, { target: { value: '   ' } });
		form = dialog.querySelector('form');
		expect(form).not.toBeNull();
		if (form === null) throw new Error('Expected the Tab title form.');
		fireEvent.submit(form);
		expect(within(dialog).getByRole('alert').textContent).toBe('Enter a name.');
		fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(change).toHaveBeenCalledOnce();
		expect(document.activeElement).toBe(titleReturnFocus);
	});

	it('names new and existing Tab groups in the group name dialog', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1)]),
		);
		const change = completingChange();
		renderWorkbench({ authority: inertWorkbenchAuthority({ change }), projectWorkspaceRuntime });

		fireEvent.contextMenu(screen.getByRole('tab', { name: 'Tab 1' }), {
			clientX: 120,
			clientY: 20,
		});
		const tabMenu = await screen.findByRole('menu', { name: 'Tab options' });
		fireEvent.click(within(tabMenu).getByRole('menuitem', { name: 'Add to group' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'New group…' }));
		let dialog = await screen.findByRole('dialog', { name: 'New Tab Group' });
		// The new group is named after the Tab the menu opened on until another name is typed.
		let nameInput = within(dialog).getByLabelText<HTMLInputElement>('New Tab Group');
		expect(nameInput.value).toBe('Tab 1');
		expect(within(dialog).getByText('Leave blank for an unnamed group.')).toBeDefined();
		fireEvent.change(nameInput, { target: { value: 'Build' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Create' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(change).toHaveBeenLastCalledWith(expect.anything(), {
			change: { color: 'blue', kind: 'tabs-grouped', tabIds: [fixtureTabId(1)], title: 'Build' },
			terminalContextId: fixtureContextId(1),
			type: 'tabs-changed',
		});
		const groupId = fixtureId(0xc01);
		act(() =>
			publishSnapshot(
				projectWorkspaceRuntime,
				workbenchSnapshot(
					2,
					{ directories: [snapshotDirectory(1)], workspaces: [snapshotWorkspace(1)] },
					[
						snapshotContext(
							1,
							[snapshotTab(1, undefined, { groupId })],
							[{ color: 'blue', groupId, label: 'Build' }],
						),
					],
				),
			),
		);

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
		expect(change).toHaveBeenLastCalledWith(expect.anything(), {
			change: { groupId, kind: 'group-updated', title: '' },
			terminalContextId: fixtureContextId(1),
			type: 'tabs-changed',
		});
	});

	it('gives the terminal context menu the panel menu composition, Maximize beside another panel', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1)]),
		);
		const createPanelTerminal = vi.fn<WorkbenchAuthority['createPanelTerminal']>(async () => {
			publishSnapshot(
				projectWorkspaceRuntime,
				oneWorkspaceSnapshot(2, [
					snapshotTab(1, snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(2))),
				]),
			);
			return {
				kind: 'completed',
				value: {
					panelId: fixturePanelId(2),
					presentationId: fixturePresentationId(2),
					tabId: fixtureTabId(1),
					terminalContextId: fixtureContextId(1),
					terminalStarted: true,
				},
			};
		});
		renderWorkbench({
			authority: inertWorkbenchAuthority({ createPanelTerminal }),
			projectWorkspaceRuntime,
		});

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
		expect(createPanelTerminal).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			direction: 'right',
			panelId: fixturePanelId(1),
			tabId: fixtureTabId(1),
			terminalContextId: fixtureContextId(1),
		});
		await waitFor(() =>
			expect(document.querySelectorAll('[data-slot="workbench-terminal-viewport"]')).toHaveLength(
				2,
			),
		);

		expect(getActionableMenuPresentation(await openTerminalMenu())).toEqual([
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
			const projectWorkspaceRuntime = createSnapshotRuntime(emptySnapshot());
			const authorityCreate = publishingCreate(projectWorkspaceRuntime, orchbeeSnapshot(2), 1);
			renderWorkbench({
				authority: inertWorkbenchAuthority({ create: authorityCreate }),
				locale,
				projectWorkspaceRuntime,
			});

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
			await waitFor(() =>
				expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
					kind: 'workspace',
					workspaceId: fixtureWorkspaceId(1),
				}),
			);
			expect(authorityCreate).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
				path: '/workspace/orchbee/',
				projectName: 'orchbee',
				type: 'project-created',
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
			const runtime = createSnapshotRuntime(emptySnapshot());
			const authorityCreate = publishingCreate(runtime, orchbeeSnapshot(2), 1);
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
			renderWorkbench({
				authority: inertWorkbenchAuthority({ create: authorityCreate }),
				locale,
				projectFolderAcquisition: {
					newFolder: capability,
					selectExistingFolder: async () => ({ kind: 'cancelled' }),
				},
				projectWorkspaceRuntime: runtime,
				shortcutPlatform: 'standard',
			});
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
			expect(authorityCreate).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
				path: '/workspace/orchbee',
				projectName: 'orchbee',
				type: 'project-created',
			});
			expect(runtime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',
				workspaceId: fixtureWorkspaceId(1),
			});
		},
	);

	it('uses a native Folder capability for atomic Project creation', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(emptySnapshot());
		const create = publishingCreate(projectWorkspaceRuntime, orchbeeSnapshot(2), 1);
		const selectExistingFolder = vi
			.fn<() => Promise<{ readonly kind: 'selected'; readonly path: string }>>()
			.mockResolvedValueOnce({ kind: 'selected', path: '/workspace/orchbee' });
		renderWorkbench({
			authority: inertWorkbenchAuthority({ create }),
			projectFolderAcquisition: { selectExistingFolder },
			projectWorkspaceRuntime,
		});

		fireEvent.click(screen.getByRole('button', { name: 'Create Project' }));
		fireEvent.click(
			within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Select Folder…' }),
		);
		await waitFor(() =>
			expect(create).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
				path: '/workspace/orchbee',
				projectName: 'orchbee',
				type: 'project-created',
			}),
		);
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(selectExistingFolder).toHaveBeenCalledOnce();
	});

	it('shows a localized bounded native Folder failure without Project mutation', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(emptySnapshot());
		renderWorkbench({
			locale: 'zh-CN',
			projectFolderAcquisition: {
				selectExistingFolder: async () => ({
					code: 'permission-denied',
					kind: 'failed',
				}),
			},
			projectWorkspaceRuntime,
		});

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
		const projectWorkspaceRuntime = createSnapshotRuntime(orchbeeSnapshot(1));
		const create = publishingCreate(
			projectWorkspaceRuntime,
			orchbeeSnapshot(2, ['Primary', 'orchbee']),
			2,
		);
		renderWorkbench({ authority: inertWorkbenchAuthority({ create }), projectWorkspaceRuntime });

		fireEvent.click(screen.getByRole('button', { name: 'Project options: orchbee' }));
		const projectMenu = await screen.findByRole('menu');
		fireEvent.click(within(projectMenu).getByRole('menuitem', { name: 'New Workspace' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'orchbee' }));

		await waitFor(() =>
			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',
				workspaceId: fixtureWorkspaceId(2),
			}),
		);
		const workspace = document.querySelector<HTMLElement>(
			`[data-workspace-id="${fixtureWorkspaceId(2)}"]`,
		);
		expect(
			workspace
				?.querySelector('[data-slot="workbench-sidebar-workspace"]')
				?.getAttribute('aria-current'),
		).toBe('page');
		expect(create).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			directoryId: fixtureDirectoryId(1),
			projectId: fixtureProjectId(1),
			type: 'workspace-created',
		});
	});

	it('composes Project and Workspace row menus as the TUI does and renames through one field', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(orchbeeSnapshot(1));
		const change = completingChange();
		const close = vi.fn<WorkbenchAuthority['close']>(async () => ({
			code: 'confirmation-required',
			kind: 'blocked',
		}));
		renderWorkbench({
			authority: inertWorkbenchAuthority({ change, close }),
			projectWorkspaceRuntime,
		});

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
		expect(close).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			type: 'workspace-close-if-idle',
			workspaceId: fixtureWorkspaceId(1),
		});
		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			name: ' Orchbee renamed',
			projectId: fixtureProjectId(1),
			type: 'project-renamed',
		});

		fireEvent.click(screen.getByRole('button', { name: 'Project options: orchbee' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Dissolve' }));
		await waitFor(() =>
			expect(change).toHaveBeenLastCalledWith(expect.anything(), {
				projectId: fixtureProjectId(1),
				type: 'project-dissolved',
			}),
		);
	}, 10_000);

	it('previews Project reorder compactly, commits on release, and restores expansion on completion or cancellation', async () => {
		const projectsIn = (revision: number, order: readonly number[]) => {
			const names = ['Alpha', 'Beta', 'Gamma'];
			return navigationSnapshot(
				revision,
				[1, 2, 3].map((sequence) => snapshotProject(sequence, names[sequence - 1])),
				[1, 2, 3].map((sequence) => ({
					label: `${names[sequence - 1] ?? ''} Workspace`,
					projectId: fixtureProjectId(sequence),
					sequence,
				})),
				{ projectOrder: order.map(fixtureProjectId) },
			);
		};
		const projectWorkspaceRuntime = createSnapshotRuntime(projectsIn(1, [1, 2, 3]));
		const change = vi.fn<WorkbenchAuthority['change']>(async () => {
			publishSnapshot(projectWorkspaceRuntime, projectsIn(2, [3, 1, 2]));
			return { kind: 'completed' };
		});
		renderWorkbench({ authority: inertWorkbenchAuthority({ change }), projectWorkspaceRuntime });

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
		const gammaItem = document.querySelector<HTMLElement>(
			`[data-project-id="${fixtureProjectId(3)}"]`,
		);
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
				.querySelector<HTMLElement>(`[data-project-id="${fixtureProjectId(1)}"]`)
				?.getAttribute('data-project-reorder-displacement'),
		).toBeNull();
		expect(
			document
				.querySelector<HTMLElement>(`[data-project-id="${fixtureProjectId(2)}"]`)
				?.getAttribute('data-project-reorder-displacement'),
		).toBe('down');

		fireEvent.pointerMove(document, {
			clientX: 14,
			clientY: 1,
			pointerId: 7,
			pointerType: 'mouse',
		});
		expect(projectItems().map((item) => item.dataset['projectId'])).toEqual([
			fixtureProjectId(1),
			fixtureProjectId(2),
			fixtureProjectId(3),
		]);
		expect(
			document
				.querySelector<HTMLElement>(`[data-project-id="${fixtureProjectId(1)}"]`)
				?.getAttribute('data-project-reorder-displacement'),
		).toBe('down');
		expect(
			document
				.querySelector<HTMLElement>(`[data-project-id="${fixtureProjectId(2)}"]`)
				?.getAttribute('data-project-reorder-displacement'),
		).toBe('down');
		expect(
			gammaRow.style.getPropertyValue('--ob-comp-workbench-navigation-reorder-translate-y'),
		).toBe('-99px');
		expect(projectWorkspaceRuntime.getSnapshot().projects.map((project) => project.id)).toEqual([
			fixtureProjectId(1),
			fixtureProjectId(2),
			fixtureProjectId(3),
		]);
		expect(projectWorkspaceRuntime.getSnapshot().expandedProjectIds).toContain(fixtureProjectId(3));

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
				fixtureProjectId(3),
				fixtureProjectId(1),
				fixtureProjectId(2),
			]);
			expect(gammaItem.getAttribute('data-dragging')).toBeNull();
			expect(
				projectItems().every((item) => item.getAttribute('data-project-reorder-active') === null),
			).toBe(true);
			expect(gammaItem.querySelector('[data-slot="workbench-sidebar-workspace"]')).not.toBeNull();
		});
		expect(gammaButton.getAttribute('aria-expanded')).toBe('true');
		expect(projectWorkspaceRuntime.getSnapshot().expandedProjectIds).toContain(fixtureProjectId(3));

		const alphaItem = document.querySelector<HTMLElement>(
			`[data-project-id="${fixtureProjectId(1)}"]`,
		);
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
				fixtureProjectId(3),
				fixtureProjectId(1),
				fixtureProjectId(2),
			]);
			expect(alphaItem.getAttribute('data-dragging')).toBeNull();
			expect(alphaItem.querySelector('[data-slot="workbench-sidebar-workspace"]')).not.toBeNull();
		});
		expect(
			screen.getByRole('button', { name: 'Collapse project: Alpha' }).getAttribute('aria-expanded'),
		).toBe('true');
		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			beforeProjectId: fixtureProjectId(1),
			projectId: fixtureProjectId(3),
			type: 'project-reordered',
		});
	});

	it('reorders Workspace siblings with scoped squeeze motion and makes the held Workspace current through release or cancellation', async () => {
		const workspacesIn = (revision: number, order: readonly number[]) => {
			const names = ['Primary', 'Feature', 'Review'];
			return navigationSnapshot(
				revision,
				[snapshotProject(1, 'Orchbee')],
				[1, 2, 3].map((sequence) => ({ label: names[sequence - 1] ?? '', sequence })),
				{ workspaceOrder: order.map(fixtureWorkspaceId) },
			);
		};
		const projectWorkspaceRuntime = createSnapshotRuntime(workspacesIn(1, [1, 2, 3]));
		projectWorkspaceRuntime.transition({
			type: 'workspace-activated',
			workspaceId: fixtureWorkspaceId(3),
		});
		const change = vi.fn<WorkbenchAuthority['change']>(async () => {
			publishSnapshot(projectWorkspaceRuntime, workspacesIn(2, [3, 1, 2]));
			return { kind: 'completed' };
		});
		const activeLocation = projectWorkspaceRuntime.getSnapshot().activeLocation;
		const expandedProjectIds = projectWorkspaceRuntime.getSnapshot().expandedProjectIds;
		renderWorkbench({ authority: inertWorkbenchAuthority({ change }), projectWorkspaceRuntime });

		const workspaceItems = () =>
			Array.from(
				document.querySelectorAll<HTMLElement>(
					`[data-project-id="${fixtureProjectId(1)}"] [data-workspace-id]`,
				),
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
		const reviewItem = document.querySelector<HTMLElement>(
			`[data-workspace-id="${fixtureWorkspaceId(3)}"]`,
		);
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
				.querySelector<HTMLElement>(`[data-workspace-id="${fixtureWorkspaceId(1)}"]`)
				?.getAttribute('data-workspace-reorder-displacement'),
		).toBe('down');
		expect(
			document
				.querySelector<HTMLElement>(`[data-workspace-id="${fixtureWorkspaceId(2)}"]`)
				?.getAttribute('data-workspace-reorder-displacement'),
		).toBe('down');
		expect(
			reviewRow.style.getPropertyValue('--ob-comp-workbench-navigation-reorder-translate-y'),
		).toBe('-81px');
		expect(
			projectWorkspaceRuntime.getSnapshot().workspaces.map((workspace) => workspace.id),
		).toEqual([fixtureWorkspaceId(1), fixtureWorkspaceId(2), fixtureWorkspaceId(3)]);

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
			).toEqual([fixtureWorkspaceId(3), fixtureWorkspaceId(1), fixtureWorkspaceId(2)]);
			expect(workspaceItems().map((item) => item.dataset['workspaceId'])).toEqual([
				fixtureWorkspaceId(3),
				fixtureWorkspaceId(1),
				fixtureWorkspaceId(2),
			]);
			expect(reviewItem.getAttribute('data-dragging')).toBeNull();
		});
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual(activeLocation);
		expect(projectWorkspaceRuntime.getSnapshot().expandedProjectIds).toEqual(expandedProjectIds);

		const primaryItem = document.querySelector<HTMLElement>(
			`[data-workspace-id="${fixtureWorkspaceId(1)}"]`,
		);
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
		// Held, a Workspace that was not current becomes current, and stays so when the drag is cancelled.
		const heldLocation = { kind: 'workspace', workspaceId: fixtureWorkspaceId(1) };
		await waitFor(() =>
			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual(heldLocation),
		);
		fireEvent.keyDown(document, { key: 'Escape' });
		fireEvent.click(primaryButton);

		await waitFor(() => {
			expect(
				projectWorkspaceRuntime.getSnapshot().workspaces.map((workspace) => workspace.id),
			).toEqual([fixtureWorkspaceId(3), fixtureWorkspaceId(1), fixtureWorkspaceId(2)]);
			expect(primaryItem.getAttribute('data-dragging')).toBeNull();
		});
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual(heldLocation);
		expect(
			screen
				.getByRole('button', { name: 'Collapse project: Orchbee' })
				.getAttribute('aria-expanded'),
		).toBe('true');
		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			beforeWorkspaceId: fixtureWorkspaceId(1),
			type: 'workspace-reordered',
			workspaceId: fixtureWorkspaceId(3),
		});
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
