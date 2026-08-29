// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../terminal-presentation';
import { Workbench } from '../Workbench';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchProjectWorktreeCapability } from '../types/workbench-project-worktree';
import type { WorkbenchTabWindowControl } from '../types/workbench-tab-window-control';
import {
	defaultWorkbenchShortcutBindings,
	type WorkbenchShortcutBindings,
} from '../utils/workbench-shortcuts';
import { findActiveTerminalFocusTarget } from '../utils/findActiveTerminalFocusTarget';

function createTestTerminalPresentationComposition(scope: string) {
	let sequence = 0;
	return createWorkbenchPreviewTerminalPresentationComposition(
		terminalPresentationId(`${scope}:initial`),
		() => terminalPresentationId(`${scope}:dynamic:${(sequence += 1)}`),
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
	it('projects native windows and routes a tab move through the host control', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('native-window-menu'),
		});
		projectWorkspaceRuntime.createTerminal({
			groupId: null,
			source: 'button',
			title: 'Terminal 2',
		});
		const tabIds = projectWorkspaceRuntime
			.getActiveTerminalContext()
			.tabModel.getSnapshot()
			.tabs.map((tab) => tab.id);
		const snapshot = {
			currentWindowId: 'desktop-window-1',
			revision: '1',
			terminalContexts: [{ tabIds, terminalContextId: 'terminal-context-1' }],
			windows: [
				{ ordinal: 1, windowId: 'desktop-window-1' },
				{ ordinal: 2, windowId: 'desktop-window-2' },
			],
		} as const;
		const tabWindowControl = {
			finishDrag: vi
				.fn<WorkbenchTabWindowControl['finishDrag']>()
				.mockResolvedValue({ kind: 'unchanged' }),
			getDragTarget: vi.fn<WorkbenchTabWindowControl['getDragTarget']>().mockReturnValue(null),
			getSnapshot: vi.fn<WorkbenchTabWindowControl['getSnapshot']>().mockReturnValue(snapshot),
			moveGroup: vi
				.fn<WorkbenchTabWindowControl['moveGroup']>()
				.mockResolvedValue({ kind: 'unchanged' }),
			moveTabs: vi
				.fn<WorkbenchTabWindowControl['moveTabs']>()
				.mockResolvedValue({ destinationWindowId: 'desktop-window-2', kind: 'completed' }),
			startDrag: vi.fn<WorkbenchTabWindowControl['startDrag']>().mockReturnValue(null),
			subscribe: vi.fn<WorkbenchTabWindowControl['subscribe']>(() => () => undefined),
			updateDrag: vi.fn<WorkbenchTabWindowControl['updateDrag']>(),
		} satisfies WorkbenchTabWindowControl;
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='apple'
						tabWindowControl={tabWindowControl}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		const sourceTab = screen.getAllByRole('tab', { name: 'system' })[0];
		if (sourceTab === undefined) throw new Error('Expected the first terminal Tab.');
		fireEvent.contextMenu(sourceTab, {
			clientX: 120,
			clientY: 20,
		});
		const moveTrigger = await screen.findByRole('menuitem', { name: 'Move tab to window' });
		fireEvent.keyDown(moveTrigger, { key: 'ArrowRight' });
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Window 2' }));

		await waitFor(() =>
			expect(tabWindowControl.moveTabs).toHaveBeenCalledWith('terminal-context-1', [tabIds[0]], {
				kind: 'existing',
				windowId: 'desktop-window-2',
			}),
		);
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
		const [alpha, beta] = projectWorkspaceRuntime.getSnapshot().projects;
		const alphaContext =
			alpha?.workspaces[0] === undefined
				? null
				: projectWorkspaceRuntime.getTerminalContext(alpha.workspaces[0].terminalContextId);
		const betaContext =
			beta?.workspaces[0] === undefined
				? null
				: projectWorkspaceRuntime.getTerminalContext(beta.workspaces[0].terminalContextId);
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
			projectId: 'project-2',
			workspaceId: 'workspace-2',
		});
	});

	it('projects current configured bindings into the tab context menu', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('tab-menu-shortcuts'),
		});
		const shortcutBindings = {
			...defaultWorkbenchShortcutBindings('apple'),
			'workbench.terminal.active.close': {
				code: 'KeyQ',
				modifiers: ['mod'],
			},
			'workbench.terminal.create': {
				code: 'KeyN',
				modifiers: ['mod', 'shift'],
			},
		} as const satisfies WorkbenchShortcutBindings;
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutBindings={shortcutBindings}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		fireEvent.contextMenu(screen.getByRole('tab', { name: 'system' }), {
			clientX: 120,
			clientY: 20,
		});
		const menu = await screen.findByRole('menu', { name: 'Tab options' });
		expect(
			Array.from(menu.querySelectorAll('[data-slot="context-menu-shortcut"]')).map(
				(shortcut) => shortcut.textContent,
			),
		).toEqual(['⇧⌘N', '⇧⌘I', '⌘Q']);
	});

	it('sets and restores the captured Tab title from the shortcut and context menu', async () => {
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

		let dialog = await screen.findByRole('dialog', { name: 'Set Tab Title' });
		let titleInput = within(dialog).getByLabelText<HTMLInputElement>('Tab title');
		expect(titleInput.value).toBe('system');
		expect(document.activeElement).toBe(titleInput);
		expect(titleInput.selectionStart).toBe(0);
		expect(titleInput.selectionEnd).toBe(titleInput.value.length);

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
		const titleMenuItem = within(menu).getByRole('menuitem', { name: /^Set Tab Title…/ });
		expect(titleMenuItem.textContent).toBe('Set Tab Title…⇧⌘I');
		fireEvent.click(titleMenuItem);
		dialog = await screen.findByRole('dialog', { name: 'Set Tab Title' });
		titleInput = within(dialog).getByLabelText<HTMLInputElement>('Tab title');
		expect(titleInput.value).toBe('Focus work');
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

	it('projects terminal context commands through the shared icon-bearing action menu', async () => {
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

		const terminalViewport = document.querySelector<HTMLElement>(
			'[data-slot="workbench-terminal-viewport"]',
		);
		expect(terminalViewport).not.toBeNull();
		if (terminalViewport === null) throw new Error('Expected a terminal Context Menu trigger.');
		const contextMenuEvent = new MouseEvent('contextmenu', {
			bubbles: true,
			cancelable: true,
			clientX: 400,
			clientY: 300,
		});
		fireEvent(terminalViewport, contextMenuEvent);

		expect(contextMenuEvent.defaultPrevented).toBe(true);
		const menu = await screen.findByRole('menu');
		expectActionableMenuItemsToHaveIcons(menu);
		expect(getActionableMenuPresentation(menu)).toEqual([
			{ icon: 'tabs', label: 'New tab⌘T' },
			{ icon: 'tabs', label: 'New tab in current directory⌥⌘Enter' },
			{ icon: 'terminal', label: 'New child terminal' },
			{ icon: 'panels', label: 'Resize Panel…⌘R' },
			{ icon: 'terminal', label: 'Broadcast Input' },
			{ icon: 'panels', label: 'Equalize Panels⌃⌘=' },
			{ icon: 'screen-full', label: 'Maximize Panel⇧⌘Enter' },
			{ icon: 'panels', label: 'Split panel' },
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
						projectId: 'project-1',
						workspaceId: 'workspace-1',
					},
					projects: [
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

	it('uses a native Folder capability for atomic Project creation and Folder addition', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('native-folder'),
		});
		const selectExistingFolder = vi
			.fn<() => Promise<{ readonly kind: 'selected'; readonly path: string }>>()
			.mockResolvedValueOnce({ kind: 'selected', path: '/workspace/orchbee' })
			.mockResolvedValueOnce({ kind: 'selected', path: '/workspace/ghostty' });
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
		await waitFor(() =>
			expect(projectWorkspaceRuntime.getSnapshot().projects[0]).toMatchObject({
				directories: [{ path: '/workspace/orchbee' }],
				name: 'orchbee',
			}),
		);
		expect(screen.queryByRole('dialog')).toBeNull();

		fireEvent.click(screen.getByRole('button', { name: 'Project options: orchbee' }));
		fireEvent.click(
			within(await screen.findByRole('menu')).getByRole('menuitem', {
				name: 'Manage Project…',
			}),
		);
		const manager = await screen.findByRole('dialog');
		const addFolder = within(manager).getByRole('button', { name: 'Add Folder' });
		expect(addFolder.querySelector('[data-icon]')?.getAttribute('data-icon')).toBe('folder-opened');
		fireEvent.click(addFolder);
		await waitFor(() =>
			expect(projectWorkspaceRuntime.getSnapshot().projects[0]?.directories).toEqual(
				expect.arrayContaining([expect.objectContaining({ path: '/workspace/ghostty' })]),
			),
		);
		expect(within(manager).queryByLabelText('Folder path')).toBeNull();
		expect(selectExistingFolder).toHaveBeenCalledTimes(2);
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

	it('prioritizes the new branch and only offers repository selection when needed', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('managed-worktree'),
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
		const discover = vi.fn<WorkbenchProjectWorktreeCapability['discover']>(async () => ({
			discovery: {
				managedRoot: '/Users/test/.orchbee-dev-scope/worktrees',
				repositories: [
					{
						defaultBaseRef: 'main',
						label: 'orchbee',
						localBaseRefs: ['compare/warp', 'main', 'release'],
						repositoryId: 'repository-orchbee',
						sourceDirectoryId: 'directory-orchbee',
						suggestedBranchName: 'orchbee/workspace-1',
						worktreeInventory: { kind: 'available', worktrees: [] },
					},
				],
			},
			kind: 'discovered',
		}));
		const create = vi.fn<WorkbenchProjectWorktreeCapability['create']>(async () => ({
			directoryId: 'managed-worktree-1',
			kind: 'created',
			workspaceId: 'workspace-2',
		}));
		const adopt = vi.fn<WorkbenchProjectWorktreeCapability['adopt']>(async () => ({
			directoryId: 'discovered-worktree-1',
			kind: 'adopted',
			workspaceId: 'workspace-2',
		}));
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						projectWorktree={{ adopt, create, discover }}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		fireEvent.click(screen.getByRole('button', { name: 'Project options: orchbee' }));
		const projectMenu = await screen.findByRole('menu');
		const newWorkspace = within(projectMenu).getByRole('menuitem', { name: 'New Workspace' });
		fireEvent.click(newWorkspace);
		const newWorktree = await screen.findByRole('menuitem', { name: 'New Worktree…' });
		fireEvent.click(newWorktree);

		const dialog = await screen.findByRole('dialog');
		await waitFor(() =>
			expect(discover).toHaveBeenCalledWith({
				preferredDirectoryId: 'directory-orchbee',
				projectId: 'project-1',
			}),
		);
		expect(within(dialog).queryByLabelText('Repository')).toBeNull();
		const base = within(dialog).getByLabelText('Base branch or commit');
		const branch = within(dialog).getByLabelText('New branch');
		expect(base).toHaveProperty('value', 'main');
		expect(branch).toHaveProperty('value', 'orchbee/workspace-1');
		expect(document.activeElement).toBe(branch);
		const location = within(dialog).getByLabelText('Managed location');
		expect(location).toHaveProperty(
			'value',
			'/Users/test/.orchbee-dev-scope/worktrees/managed-worktree-…',
		);
		expect(location).toHaveProperty('readOnly', true);
		expect(
			[...dialog.querySelectorAll('input')]
				.map((input) => input.id)
				.filter((id) => id.startsWith('project-worktree-')),
		).toEqual(['project-worktree-branch', 'project-worktree-base', 'project-worktree-location']);
		expect(
			within(dialog)
				.getByRole('checkbox', { name: 'Create a Workspace for this Worktree' })
				.getAttribute('aria-checked'),
		).toBe('true');
		fireEvent.mouseDown(base);
		fireEvent.input(base, { inputType: 'insertText', target: { value: 'warp' } });
		await waitFor(() => {
			expect(base).toHaveProperty('value', 'warp');
			expect(screen.getByRole('option', { name: 'compare/warp' })).toBeDefined();
			expect(screen.queryByRole('option', { name: 'main' })).toBeNull();
		});
		const filteredBase = screen.getByRole('option', { name: 'compare/warp' });
		fireEvent.click(filteredBase);
		expect(base).toHaveProperty('value', 'compare/warp');
		fireEvent.input(base, { inputType: 'insertText', target: { value: '8d34c51' } });
		expect(
			await screen.findByText(
				'No matching local branch. The entered branch or commit will be used.',
			),
		).toBeDefined();
		fireEvent.change(branch, { target: { value: 'feature/worktree-ui' } });
		const form = dialog.querySelector('form');
		if (form === null) throw new Error('Expected the Worktree creation form.');
		fireEvent.submit(form);

		await waitFor(() =>
			expect(create).toHaveBeenCalledWith({
				baseRef: '8d34c51',
				branchName: 'feature/worktree-ui',
				createWorkspace: true,
				projectId: 'project-1',
				repositoryId: 'repository-orchbee',
				sourceDirectoryId: 'directory-orchbee',
			}),
		);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());

		discover.mockResolvedValue({
			discovery: {
				managedRoot: '/Users/test/.orchbee-dev-scope/worktrees',
				repositories: [
					{
						defaultBaseRef: 'main',
						label: 'orchbee',
						localBaseRefs: ['main'],
						repositoryId: 'repository-orchbee',
						sourceDirectoryId: 'directory-orchbee',
						suggestedBranchName: 'orchbee/workspace-1',
						worktreeInventory: { kind: 'available', worktrees: [] },
					},
					{
						defaultBaseRef: 'stable',
						label: 'ghostty',
						localBaseRefs: ['stable'],
						repositoryId: 'repository-ghostty',
						sourceDirectoryId: 'directory-ghostty',
						suggestedBranchName: 'ghostty/workspace-1',
						worktreeInventory: { kind: 'available', worktrees: [] },
					},
				],
			},
			kind: 'discovered',
		});
		fireEvent.click(screen.getByRole('button', { name: 'Project options: orchbee' }));
		fireEvent.click(
			within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'New Workspace' }),
		);
		fireEvent.click(await screen.findByRole('menuitem', { name: 'New Worktree…' }));
		const multipleRepositoryDialog = await screen.findByRole('dialog');
		const repository = await within(multipleRepositoryDialog).findByLabelText('Repository');
		expect(repository.getAttribute('data-slot')).toBe('select-trigger');
		expect(repository.textContent).toContain('orchbee');
		fireEvent.click(repository);
		const ghosttyRepository = await screen.findByRole('option', { name: 'ghostty' });
		fireEvent.pointerDown(ghosttyRepository, { pointerType: 'mouse' });
		fireEvent.click(ghosttyRepository, { detail: 1 });
		expect(repository.textContent).toContain('ghostty');
		expect(within(multipleRepositoryDialog).getByLabelText('New branch')).toHaveProperty(
			'value',
			'ghostty/workspace-1',
		);
		expect(within(multipleRepositoryDialog).getByLabelText('Base branch or commit')).toHaveProperty(
			'value',
			'stable',
		);
	});

	it('groups existing Worktrees by Project repository and adopts one directly from New Workspace', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal',
			terminalPresentation: createTestTerminalPresentationComposition('existing-worktree-menu'),
		});
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-folder-1',
				kind: 'folder',
				name: 'folder-1',
				path: '/projects/folder-1',
				placement: { kind: 'local' },
				source: 'selected',
			},
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'project',
			terminalTitle: 'Terminal',
			type: 'project-created',
		});
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-folder-2',
				kind: 'folder',
				name: 'folder-2',
				path: '/projects/folder-2',
				placement: { kind: 'local' },
				source: 'selected',
			},
			projectId: 'project-1',
			type: 'directory-added',
		});
		const adopt = vi.fn<WorkbenchProjectWorktreeCapability['adopt']>(async () => ({
			directoryId: 'discovered-worktree-folder-2-1',
			kind: 'adopted',
			workspaceId: 'workspace-2',
		}));
		const discover = vi.fn<WorkbenchProjectWorktreeCapability['discover']>(async () => ({
			discovery: {
				managedRoot: '/managed/worktrees',
				repositories: [
					{
						defaultBaseRef: 'main',
						label: 'folder-1',
						localBaseRefs: ['main'],
						repositoryId: 'repository-folder-1',
						sourceDirectoryId: 'directory-folder-1',
						suggestedBranchName: 'folder-1/workspace-1',
						worktreeInventory: {
							kind: 'available',
							worktrees: [{ label: 'feature/folder-1', worktreeId: 'worktree-folder-1' }],
						},
					},
					{
						defaultBaseRef: 'main',
						label: 'folder-2',
						localBaseRefs: ['main'],
						repositoryId: 'repository-folder-2',
						sourceDirectoryId: 'directory-folder-2',
						suggestedBranchName: 'folder-2/workspace-1',
						worktreeInventory: {
							kind: 'available',
							worktrees: [
								{ label: 'feature/folder-2-1', worktreeId: 'worktree-folder-2-1' },
								{ label: 'feature/folder-2-2', worktreeId: 'worktree-folder-2-2' },
							],
						},
					},
				],
			},
			kind: 'discovered',
		}));
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<Workbench
						actionRuntime={new ActionRuntime()}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						projectWorktree={{
							adopt,
							create: vi.fn<WorkbenchProjectWorktreeCapability['create']>(),
							discover,
						}}
						shortcutPlatform='apple'
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		);

		fireEvent.click(screen.getByRole('button', { name: 'Project options: project' }));
		await waitFor(() => expect(discover).toHaveBeenCalledOnce());
		const rootMenu = await screen.findByRole('menu');
		fireEvent.click(within(rootMenu).getByRole('menuitem', { name: 'New Workspace' }));
		const worktree = await screen.findByRole('menuitem', { name: 'feature/folder-2-1' });
		const submenu = worktree.closest('[role="menu"]');
		if (!(submenu instanceof HTMLElement)) throw new Error('Expected the New Workspace submenu.');
		expect(within(submenu).getAllByText('folder-1')).toHaveLength(2);
		expect(within(submenu).getAllByText('folder-2')).toHaveLength(2);
		expect(within(submenu).getByRole('menuitem', { name: 'feature/folder-1' })).toBeDefined();
		expect(within(submenu).getByRole('menuitem', { name: 'feature/folder-2-2' })).toBeDefined();

		fireEvent.click(worktree);
		expect(adopt).toHaveBeenCalledWith({
			projectId: 'project-1',
			repositoryId: 'repository-folder-2',
			sourceDirectoryId: 'directory-folder-2',
			worktreeId: 'worktree-folder-2-1',
		});
	});

	it('omits redundant primary controls when a Project has one Folder', async () => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('workspace-primary'),
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

		fireEvent.click(screen.getByRole('button', { name: 'Project options: orchbee' }));
		fireEvent.click(
			within(await screen.findByRole('menu')).getByRole('menuitem', {
				name: 'Manage Project…',
			}),
		);
		const dialog = await screen.findByRole('dialog');
		expect(within(dialog).queryByText('Primary folder')).toBeNull();
		expect(within(dialog).queryByRole('button', { name: 'Set as primary' })).toBeNull();
		const folderName = within(dialog).getByText('orchbee');
		expect(folderName.getAttribute('data-slot')).toBe('tooltip-trigger');
		const addFolder = within(dialog).getByRole('button', { name: 'Add Folder' });
		expect(addFolder.querySelector('[data-icon]')?.getAttribute('data-icon')).toBe('new-folder');
	});

	it('removes an active primary Folder without asking for a replacement', async () => {
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
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-xterm',
				kind: 'folder',
				name: 'xterm.js',
				path: '/workspace/xterm.js',
				placement: { kind: 'local' },
				source: 'selected',
			},
			projectId: 'project-1',
			type: 'directory-added',
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
		expect(workspaceMenuPresentation.map(({ label }) => label)).toEqual([
			'Duplicate Workspace',
			'Manage Workspace…',
			'Delete Workspace',
		]);
		fireEvent.click(within(workspaceMenu).getByRole('menuitem', { name: 'Delete Workspace' }));
		const deleteWorkspaceTitle = await screen.findByText('Delete Workspace?');
		const confirmationDialog = deleteWorkspaceTitle.closest<HTMLElement>(
			'[data-slot="alert-dialog-content"]',
		);
		expect(confirmationDialog).not.toBeNull();
		if (confirmationDialog === null) throw new Error('Expected a modal confirmation Alert Dialog.');
		expect(document.querySelector('[data-slot="alert-dialog-overlay"]')).not.toBeNull();
		expect(document.querySelector('[data-slot="workbench-confirmation-popover"]')).toBeNull();
		const confirmationCancel = within(confirmationDialog).getByRole('button', { name: 'Cancel' });
		const confirmationSubmit = within(confirmationDialog).getByRole('button', {
			name: 'Delete Workspace',
		});
		expect(within(confirmationCancel).getByText('Esc')).toBeDefined();
		expect(within(confirmationSubmit).getByText('Enter')).toBeDefined();
		await waitFor(() => expect(document.activeElement).toBe(confirmationSubmit));
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
		fireEvent.click(
			within(workspaceContextMenu).getByRole('menuitem', { name: 'Manage Workspace…' }),
		);
		const workspaceManager = await screen.findByRole('dialog');
		expect(workspaceManager.getAttribute('data-slot')).toBe('workbench-workspace-manager');
		expect(within(workspaceManager).getByRole('button', { name: 'Close' })).toBeDefined();
		const workspaceNameInput = within(workspaceManager).getByLabelText('Name');
		expect(workspaceNameInput.getAttribute('placeholder')).toBe('Enter a workspace name');
		const workspaceCancel = within(workspaceManager).getByRole('button', { name: 'Cancel' });
		const workspaceSave = within(workspaceManager).getByRole('button', { name: 'Save' });
		expect(within(workspaceCancel).getByText('Esc')).toBeDefined();
		expect(within(workspaceSave).getByText('Enter')).toBeDefined();
		expect(workspaceSave.getAttribute('type')).toBe('submit');
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
		const contextSubmenu = within(projectContextMenu).getByRole('menuitem', {
			name: 'New Workspace',
		});
		expect(contextSubmenu.className).toContain('data-popup-open:bg-accent');
		const projectContextMenuPresentation = getActionableMenuPresentation(projectContextMenu);
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
		const newWorkspaceSubmenu = within(projectMenu).getByRole('menuitem', {
			name: 'New Workspace',
		});
		expect(newWorkspaceSubmenu.querySelector('svg')).not.toBeNull();
		expect(newWorkspaceSubmenu.className).toContain('data-popup-open:bg-accent');
		expect(newWorkspaceSubmenu.className).toContain('[&>svg]:text-muted-foreground');
		expect(newWorkspaceSubmenu.className).toContain('size-3.5');
		expect(getActionableMenuPresentation(projectMenu)).toEqual(projectContextMenuPresentation);
		fireEvent.click(within(projectMenu).getByRole('menuitem', { name: 'Manage Project…' }));
		const dialog = await screen.findByRole('dialog');
		expect(within(dialog).queryByRole('combobox')).toBeNull();
		expect(within(dialog).getByRole('button', { name: 'Close' })).toBeDefined();
		const projectNameInput = within(dialog).getByLabelText('Project name');
		expect(projectNameInput.getAttribute('placeholder')).toBe('Enter a project name');
		const projectCancel = within(dialog).getByRole('button', { name: 'Cancel' });
		const projectSave = within(dialog).getByRole('button', { name: 'Save' });
		expect(within(projectCancel).getByText('Esc')).toBeDefined();
		expect(within(projectSave).getByText('Enter')).toBeDefined();
		expect(projectSave.getAttribute('type')).toBe('submit');
		expect(within(dialog).getByText('Primary folder')).toBeDefined();
		expect(within(dialog).getByRole('button', { name: 'Set as primary' })).toBeDefined();
		expect(within(dialog).getByText('xterm.js').getAttribute('data-slot')).toBe('tooltip-trigger');

		fireEvent.click(within(dialog).getByRole('button', { name: 'Add Folder' }));
		let folderPathInput = within(dialog).getByLabelText('Folder path');
		expect(folderPathInput.getAttribute('placeholder')).toBe('/path/to/project');
		expect(document.activeElement).toBe(folderPathInput);
		expect(within(dialog).getByRole('button', { name: 'Save' })).toBe(projectSave);
		expect(within(dialog).getAllByRole('button', { name: 'Cancel' })).toHaveLength(2);
		let folderAddRow = folderPathInput.closest<HTMLElement>('li');
		expect(folderAddRow).not.toBeNull();
		if (folderAddRow === null) throw new Error('Expected the inline Folder add row.');
		expect(within(folderAddRow).getByRole('button', { name: 'Add' })).toHaveProperty(
			'disabled',
			true,
		);
		fireEvent.change(folderPathInput, { target: { value: ' /workspace/orchbee ' } });
		expect(within(dialog).getByText('This folder is already in the Project.')).toBeDefined();
		expect(within(folderAddRow).getByRole('button', { name: 'Add' })).toHaveProperty(
			'disabled',
			true,
		);
		fireEvent.keyDown(folderPathInput, { key: 'Escape' });
		expect(screen.getByRole('dialog')).toBe(dialog);
		expect(within(dialog).queryByLabelText('Folder path')).toBeNull();
		expect(within(dialog).getByRole('button', { name: 'Save' })).toBe(projectSave);

		fireEvent.click(within(dialog).getByRole('button', { name: 'Add Folder' }));
		folderPathInput = within(dialog).getByLabelText('Folder path');
		folderAddRow = folderPathInput.closest<HTMLElement>('li');
		expect(folderAddRow).not.toBeNull();
		if (folderAddRow === null) throw new Error('Expected the inline Folder add row.');
		const folderAddCancel = within(folderAddRow).getByRole('button', { name: 'Cancel' });
		const folderAddSubmit = within(folderAddRow).getByRole('button', { name: 'Add' });
		expect(folderAddCancel.getAttribute('data-slot')).toBe('tooltip-trigger');
		expect(folderAddCancel.querySelector('[data-icon="close"]')).not.toBeNull();
		expect(folderAddSubmit.getAttribute('data-slot')).toBe('tooltip-trigger');
		expect(folderAddSubmit.querySelector('[data-icon="check"]')).not.toBeNull();
		fireEvent.change(folderPathInput, { target: { value: ' /workspace/ghostty/ ' } });
		fireEvent.keyDown(folderPathInput, { key: 'Enter' });
		await waitFor(() => {
			expect(projectWorkspaceRuntime.getSnapshot().projects[0]).toMatchObject({
				directories: [
					{ id: 'directory-orchbee' },
					{ id: 'directory-xterm' },
					{ name: 'ghostty', path: '/workspace/ghostty/' },
				],
				primaryDirectoryId: 'directory-orchbee',
				workspaces: [{ primaryDirectoryId: 'directory-orchbee' }],
			});
		});
		expect(screen.getByRole('dialog')).toBe(dialog);
		expect(within(dialog).getByText('ghostty')).toBeDefined();
		expect(within(dialog).queryByLabelText('Folder path')).toBeNull();

		fireEvent.click(within(dialog).getByRole('button', { name: 'Remove folder: orchbee' }));
		await waitFor(() => {
			expect(projectWorkspaceRuntime.getSnapshot().projects[0]).toMatchObject({
				primaryDirectoryId: 'directory-xterm',
				workspaces: [{ primaryDirectoryId: 'directory-xterm' }],
			});
		});
		expect(within(dialog).getByText('xterm.js')).toBeDefined();
		expect(within(dialog).getByText('Primary folder')).toBeDefined();
		fireEvent.change(projectNameInput, { target: { value: '  Orchbee renamed  ' } });
		const projectForm = dialog.querySelector('form');
		expect(projectForm).not.toBeNull();
		if (projectForm === null) throw new Error('Expected the Project management form to exist.');
		fireEvent.submit(projectForm);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]?.name).toBe('Orchbee renamed');

		fireEvent.click(screen.getByRole('button', { name: 'Project options: Orchbee renamed' }));
		fireEvent.click(
			within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Remove Project' }),
		);
		const removeProjectTitle = await screen.findByText('Remove Project?');
		const removeProjectDialog = removeProjectTitle.closest<HTMLElement>(
			'[data-slot="alert-dialog-content"]',
		);
		expect(removeProjectDialog).not.toBeNull();
		expect(document.querySelector('[data-slot="alert-dialog-overlay"]')).not.toBeNull();
		if (removeProjectDialog === null) throw new Error('Expected a Project removal Alert Dialog.');
		fireEvent.click(within(removeProjectDialog).getByRole('button', { name: 'Cancel' }));
		await waitFor(() => expect(screen.queryByText('Remove Project?')).toBeNull());
		expect(projectWorkspaceRuntime.getSnapshot().projects).toHaveLength(1);
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
			projectWorkspaceRuntime
				.getSnapshot()
				.projects[0]?.workspaces.map((workspace) => workspace.id),
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
				projectWorkspaceRuntime
					.getSnapshot()
					.projects[0]?.workspaces.map((workspace) => workspace.id),
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
				projectWorkspaceRuntime
					.getSnapshot()
					.projects[0]?.workspaces.map((workspace) => workspace.id),
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
