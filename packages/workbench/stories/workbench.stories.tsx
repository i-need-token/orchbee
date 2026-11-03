import type { ComponentProps } from 'react';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { ActionRuntime } from '@orchbee/actions';
import {
	Workbench,
	WorkbenchProjectWorkspaceRuntime,
	createWorkbenchPreviewTerminalPresentationComposition,
} from '@orchbee/workbench';
import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchEnvironmentProjectConnectionCapability,
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalPresentationCommand,
	WorkbenchTerminalSurfaceProps,
	WorkbenchWorkspaceLocation,
	WorkbenchWorkspaceLocationSource,
} from '@orchbee/workbench';
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench-client-protocol';

const meta = {
	title: 'Workbench',
	component: Workbench,
	parameters: {
		layout: 'fullscreen',
	},
} satisfies Meta<typeof Workbench>;

export default meta;

type Story = StoryObj<typeof meta>;

const unavailableSearchProvider: NonNullable<
	ComponentProps<typeof Workbench>['searchProviders']
>[number] = {
	filters: ['all', 'agents'],
	getDefaultSections: () => Promise.reject(new Error('Story provider unavailable')),
	id: 'remote-agents',
	label: 'Remote agents',
	priority: 20,
	resultLimit: 5,
	search: () => Promise.reject(new Error('Story provider unavailable')),
};

const createProjectWorkspaceRuntime = (titles: readonly [string, ...string[]]) => {
	let presentationSequence = 0;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/workspace/orchbee',
		standaloneTerminalTitle: titles[0],
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('storybook:standalone:initial'),
			() => terminalPresentationId(`storybook:dynamic:${(presentationSequence += 1)}`),
		),
	});
	for (const title of titles.slice(1)) {
		runtime.createTerminal({
			groupId: null,
			source: 'button',
			title,
		});
	}
	return runtime;
};

const createPopulatedProjectWorkspaceRuntime = () => {
	const runtime = createProjectWorkspaceRuntime(['Terminal 1']);
	runtime.transition({
		directory: {
			available: true,
			git: {
				branch: 'main',
				head: 'preview',
				repositoryId: 'orchbee',
				worktree: false,
			},
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
	runtime.transition({
		directory: {
			available: true,
			git: {
				branch: 'feature/project-workspace',
				head: 'preview-feature',
				repositoryId: 'orchbee',
				worktree: true,
			},
			id: 'directory-orchbee-feature',
			kind: 'worktree',
			name: 'orchbee-feature',
			path: '/workspace/orchbee-feature',
			placement: { kind: 'local' },
			source: 'discovered',
		},
		resourceScopeId: 'resource-scope-1',
		type: 'directory-added',
	});
	runtime.transition({
		directoryId: 'directory-orchbee-feature',
		launchConfigurationId: null,
		name: 'Project Workspace',
		resourceScopeId: 'resource-scope-1',
		projectId: 'project-1',
		terminalTitle: 'Terminal 1',
		type: 'workspace-created',
	});
	return runtime;
};

const createSidebarFocusRevealRuntime = (collapsed: boolean) => {
	const runtime = createPopulatedProjectWorkspaceRuntime();
	for (let index = 1; index <= 24; index += 1) {
		runtime.transition({
			directory: {
				available: true,
				git: null,
				id: `directory-focus-${index}`,
				kind: 'folder',
				name: `focus-${index}`,
				path: `/workspace/focus-${index}`,
				placement: { kind: 'local' },
				source: 'selected',
			},
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: `Focus Project ${index}`,
			terminalTitle: `Focus Terminal ${index}`,
			type: 'project-created',
		});
	}
	if (collapsed) {
		const project = runtime.getSnapshot().projects.at(-1);
		if (project === undefined) throw new Error('Expected the final focus-reveal Project.');
		runtime.transition({ projectId: project.id, type: 'project-expansion-toggled' });
	}
	return runtime;
};

/** `crowded` adds a Workspace holding more Agents than a folded status line shows. */
const createWorkspaceAgentOverviewStoryFixture = ({ crowded = false } = {}) => {
	const runtime = createPopulatedProjectWorkspaceRuntime();
	runtime.transition({
		directoryId: 'directory-orchbee',
		launchConfigurationId: null,
		name: 'Batch Tasks',
		resourceScopeId: 'resource-scope-1',
		projectId: 'project-1',
		terminalTitle: 'Batch Agent 1',
		type: 'workspace-created',
	});
	runtime.transition({
		directoryId: 'directory-orchbee',
		launchConfigurationId: null,
		name: 'Docs',
		resourceScopeId: 'resource-scope-1',
		projectId: 'project-1',
		terminalTitle: 'Documentation',
		type: 'workspace-created',
	});
	const workspaces = runtime.getSnapshot().workspaces;
	const primary = workspaces.find((workspace) => workspace.name === 'Primary');
	const review = workspaces.find((workspace) => workspace.name === 'Project Workspace');
	const batch = workspaces.find((workspace) => workspace.name === 'Batch Tasks');
	if (primary === undefined || review === undefined || batch === undefined) {
		throw new Error('Expected all Agent overview Storybook Workspaces.');
	}
	const primaryPresentations = createStoryWorkspacePresentations(runtime, primary.id, 1);
	const reviewPresentations = createStoryWorkspacePresentations(runtime, review.id, 2);
	const batchPresentations = createStoryWorkspacePresentations(runtime, batch.id, 3);
	const crowdedAgents = crowded ? createCrowdedStoryWorkspaceAgents(runtime) : [];
	runtime.transition({
		type: 'workspace-activated',
		workspaceId: review.id,
	});
	return {
		agentObservationSource: createStoryAgentSource([
			{ agent: 'codex', presentationId: primaryPresentations[0], state: 'working' },
			{ agent: 'claude', presentationId: reviewPresentations[0], state: 'blocked' },
			{ agent: 'codex', presentationId: reviewPresentations[1], state: 'done' },
			{ agent: 'codex', presentationId: batchPresentations[0], state: 'working' },
			{ agent: 'claude', presentationId: batchPresentations[1], state: 'error' },
			{ agent: 'gemini', presentationId: batchPresentations[2], state: 'idle' },
			...crowdedAgents,
		]),
		projectWorkspaceRuntime: runtime,
	};
};

/** A Workspace of seven Agents, so its folded line shows four states and `+3`. */
function createCrowdedStoryWorkspaceAgents(runtime: WorkbenchProjectWorkspaceRuntime) {
	runtime.transition({
		directoryId: 'directory-orchbee',
		launchConfigurationId: null,
		name: 'Nightly Agents',
		resourceScopeId: 'resource-scope-1',
		projectId: 'project-1',
		terminalTitle: 'Nightly Agent 1',
		type: 'workspace-created',
	});
	const nightly = runtime
		.getSnapshot()
		.workspaces.find((workspace) => workspace.name === 'Nightly Agents');
	if (nightly === undefined) throw new Error('Expected the crowded Storybook Workspace.');
	const presentations = createStoryWorkspacePresentations(runtime, nightly.id, 7);
	const states = ['working', 'idle', 'blocked', 'working', 'unknown', 'done', 'error'] as const;
	return states.map((state, index) => ({
		agent: index % 2 === 0 ? 'codex' : 'claude',
		presentationId: presentations[index],
		state,
	}));
}

function createStoryWorkspacePresentations(
	runtime: WorkbenchProjectWorkspaceRuntime,
	workspaceId: string,
	count: number,
): readonly string[] {
	const workspace = runtime
		.getSnapshot()
		.workspaces.find((candidate) => candidate.id === workspaceId);
	if (workspace === undefined) {
		throw new Error('Expected the Storybook Workspace.');
	}
	runtime.transition({ type: 'workspace-activated', workspaceId });
	for (let index = 1; index < count; index += 1) {
		runtime.createTerminal({
			groupId: null,
			source: 'programmatic',
			title: `Agent Terminal ${index + 1}`,
		});
	}
	const context = runtime.getTerminalContext(workspace.terminalContextId);
	if (context === null) throw new Error('Expected the Storybook terminal context.');
	const tabs = context.tabModel.getSnapshot().tabs.slice(0, count);
	for (const [index, tab] of tabs.entries()) {
		context.tabModel.updateTab(tab.id, { title: `${workspace.name} ${index + 1}` });
	}
	const panelsByTabId = new Map(
		context.panelWorkspaceStore.getSnapshot().tabs.map((tab) => [tab.tabId, tab] as const),
	);
	return tabs.map((tab) => {
		const panelTab = panelsByTabId.get(tab.id);
		if (panelTab === undefined) throw new Error('Expected the Storybook Panel topology.');
		if (panelTab.root.kind !== 'panel') throw new Error('Expected one Storybook terminal Panel.');
		return panelTab.root.primaryPresentation.presentationId;
	});
}

/** A Server that observes these Agents and can report each of them again. */
function createStoryAgentSource(
	entries: readonly {
		readonly agent: string;
		readonly presentationId: string | undefined;
		readonly state: WorkbenchAgentObservation['state'];
	}[],
) {
	const presentations = new Map<string, WorkbenchAgentObservation>();
	for (const { agent, presentationId, state } of entries) {
		if (presentationId === undefined) throw new Error('Expected a Storybook Agent presentation.');
		presentations.set(presentationId, { agent, binding: presentationId, sequence: 1n, state });
	}
	let snapshot: WorkbenchAgentSnapshot = { complete: true, presentations };
	const listeners = new Set<() => void>();
	return {
		getSnapshot: () => snapshot,
		/** Reports every Agent again with a newer sequence. */
		reportAgain: () => {
			snapshot = {
				complete: true,
				presentations: new Map(
					[...snapshot.presentations].map(([id, observation]) => [
						id,
						{ ...observation, sequence: observation.sequence + 1n },
					]),
				),
			};
			for (const listener of listeners) listener();
		},
		subscribe: (listener: () => void) => {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		},
	};
}

/** A Server that reports where the named Workspaces stand. */
function createStoryWorkspaceLocationSource(
	runtime: WorkbenchProjectWorkspaceRuntime,
	locations: Readonly<Record<string, WorkbenchWorkspaceLocation>>,
): WorkbenchWorkspaceLocationSource {
	const snapshot = new Map(
		runtime.getSnapshot().workspaces.flatMap((workspace) => {
			const location = locations[workspace.name];
			return location === undefined ? [] : [[workspace.id, location] as const];
		}),
	);
	return { getSnapshot: () => snapshot, subscribe: () => () => undefined };
}

const createVisualPanelSelectionRuntime = () => {
	const runtime = createProjectWorkspaceRuntime(['Terminal 1']);
	const store = runtime.getActiveTerminalContext().panelWorkspaceStore;
	const tabId = store.getActiveTabId();
	if (tabId === null) throw new Error('Expected an active terminal Tab.');
	for (let index = 0; index < 2; index += 1) {
		const tab = store.getSnapshot().tabs.find((candidate) => candidate.tabId === tabId);
		if (tab === undefined) throw new Error('Expected the active terminal Tab.');
		store.transition({
			direction: 'right',
			newTerminalTitle: `Terminal ${index + 2}`,
			panelId: tab.focusedPanelId,
			presentation: store.createTerminalPresentation(),
			tabId,
			type: 'panel-split',
		});
	}
	return runtime;
};

const createBroadcastPanelRuntime = () => {
	let operationSequence = 0;
	let presentationSequence = 0;
	let removalSequence = 0;
	const pendingCommands: WorkbenchTerminalPresentationCommand[] = [];
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/workspace/orchbee',
		standaloneTerminalTitle: 'Terminal 1',
		terminalPresentation: {
			createOperationId: () =>
				terminalPresentationOperationId(`storybook:broadcast:operation:${++operationSequence}`),
			createPresentationId: () =>
				terminalPresentationId(`storybook:broadcast:terminal:${++presentationSequence}`),
			createRemovalId: () =>
				terminalPresentationRemovalId(`storybook:broadcast:removal:${++removalSequence}`),
			dispatchCommands: (commands) => pendingCommands.push(...commands),
			initialPresentationId: terminalPresentationId('storybook:broadcast:terminal:initial'),
			kind: 'managed',
		},
	});
	const settleProvisioning = (): void => {
		for (const command of pendingCommands.splice(0)) {
			if (command.type !== 'terminal-provision-requested') continue;
			runtime.applyTerminalPresentationEvent({
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-provision-completed',
			});
		}
	};
	settleProvisioning();
	const store = runtime.getActiveTerminalContext().panelWorkspaceStore;
	const tabId = store.getActiveTabId();
	if (tabId === null) throw new Error('Expected an active terminal Tab.');
	for (let index = 0; index < 2; index += 1) {
		const tab = store.getSnapshot().tabs.find((candidate) => candidate.tabId === tabId);
		if (tab === undefined) throw new Error('Expected the active terminal Tab.');
		store.transition({
			direction: 'right',
			newTerminalTitle: `Terminal ${index + 2}`,
			panelId: tab.focusedPanelId,
			presentation: store.createTerminalPresentation(),
			tabId,
			type: 'panel-split',
		});
		settleProvisioning();
	}
	return runtime;
};

function BroadcastStoryTerminalSurface({
	bindBroadcastInputSink,
	presentation,
}: WorkbenchTerminalSurfaceProps) {
	const [received, setReceived] = useState('Waiting for broadcast input');
	const sink = useMemo(
		() => ({
			getReadiness: () => ({
				kind: 'ready' as const,
				revision: `${presentation.presentationId}:storybook-ready`,
			}),
			send: async (input: WorkbenchTerminalBroadcastInput) => {
				setReceived(input.kind === 'text' ? input.text : `[${input.key}]`);
				return { kind: 'sent' as const };
			},
			subscribe: () => ({ dispose: () => undefined }),
		}),
		[presentation.presentationId],
	);
	useEffect(() => {
		const registration = bindBroadcastInputSink(sink);
		return () => registration.dispose();
	}, [bindBroadcastInputSink, sink]);
	return (
		<div
			style={{
				background: '#10151d',
				color: '#cdd6e0',
				display: 'grid',
				fontFamily: 'monospace',
				height: '100%',
				padding: '1rem',
				placeContent: 'center',
			}}
		>
			<div>{presentation.title}</div>
			<div>{received}</div>
		</div>
	);
}

function FocusStoryTerminalSurface({
	autoFocus,
	focusRequest,
	onPresentationReady,
	onTerminalEdit,
	presentation,
	presentationMode,
}: WorkbenchTerminalSurfaceProps) {
	const inputReference = useRef<HTMLTextAreaElement>(null);
	useLayoutEffect(() => {
		if (presentationMode === 'retained') return;
		onPresentationReady('display-ready');
		onPresentationReady('activation-ready');
	}, [onPresentationReady, presentationMode]);
	useEffect(() => {
		if (autoFocus || focusRequest > 0) inputReference.current?.focus();
	}, [autoFocus, focusRequest, presentation.presentationId]);
	return (
		<textarea
			aria-label={`Terminal input: ${presentation.title}`}
			data-slot='storybook-terminal-input'
			onInput={onTerminalEdit}
			ref={inputReference}
			style={{
				background: '#10151d',
				border: 0,
				color: '#cdd6e0',
				fontFamily: 'monospace',
				height: '100%',
				padding: '1rem',
				resize: 'none',
				width: '100%',
			}}
			defaultValue={`${presentation.title}\n$ `}
		/>
	);
}

const createRemoteProjectWorkspaceRuntime = () => {
	const runtime = createProjectWorkspaceRuntime(['Terminal 1']);
	runtime.transition({
		directory: {
			available: false,
			git: null,
			id: 'directory-remote-orchbee',
			kind: 'folder',
			name: 'orchbee',
			path: '/srv/orchbee',
			placement: {
				environmentId: 'environment:00000000000000000000000000000001',
				kind: 'environment',
			},
			source: 'selected',
		},
		projectName: 'Remote orchbee',
		type: 'environment-project-registered',
	});
	runtime.transition({
		directoryId: 'directory-remote-orchbee',
		resourceScopeId: 'resource-scope-1',
		projectId: 'project-1',
		type: 'environment-workspace-registered',
		workspaceName: 'Primary',
	});
	return runtime;
};

const disconnectedRemoteProject: WorkbenchEnvironmentProjectConnectionCapability = {
	connect: () => undefined,
	disconnect: () => undefined,
	getSnapshot: () => [{ projectId: 'project-1', state: 'disconnected' }],
	subscribe: () => () => undefined,
};

export const PanelWorkspace: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createProjectWorkspaceRuntime([
			'Terminal 1',
			'Terminal 2',
			'Terminal 3',
		]),
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const PanelVisualSelection: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createVisualPanelSelectionRuntime(),
		shortcutPlatform: 'apple',
	},
	play: () => {
		document.body.dispatchEvent(
			new KeyboardEvent('keydown', {
				bubbles: true,
				code: 'F7',
				ctrlKey: true,
				shiftKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const PanelVisualSwapSelection: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createVisualPanelSelectionRuntime(),
		shortcutPlatform: 'apple',
	},
	play: () => {
		document.body.dispatchEvent(
			new KeyboardEvent('keydown', {
				bubbles: true,
				code: 'F8',
				ctrlKey: true,
				shiftKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const PanelBroadcastInput: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createBroadcastPanelRuntime(),
		shortcutPlatform: 'apple',
		terminalSurface: BroadcastStoryTerminalSurface,
	},
	play: async ({ canvasElement }) => {
		const viewport = canvasElement.querySelector<HTMLElement>(
			'[data-focused="true"] [data-slot="workbench-terminal-viewport"]',
		);
		viewport?.focus();
		viewport?.dispatchEvent(
			new MouseEvent('contextmenu', {
				bubbles: true,
				cancelable: true,
				clientX: 40,
				clientY: 40,
			}),
		);
		await new Promise((resolve) => globalThis.setTimeout(resolve, 50));
		const broadcastItem = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
			(item) => item.textContent?.includes('Broadcast Input') === true,
		);
		broadcastItem?.click();
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const SimplifiedChineseSidebar: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createProjectWorkspaceRuntime(['终端 1', '终端 2', '终端 3']),
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const NativeProjectFolderAcquisition: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectFolderAcquisition: {
			newFolder: {
				signal: new AbortController().signal,
				acknowledge: () => undefined,
				releaseParent: () => undefined,
				selectParent: async () => ({
					kind: 'parent-selected',
					parent: { path: '/workspace', selectionId: 'story-parent' },
				}),
				create: async (parent, name) => ({
					kind: 'created',
					path: `${parent.path}/${name}`,
					requestId: 'story-create',
				}),
				confirmProject: async () => true,
			},
			selectExistingFolder: () =>
				Promise.resolve({ kind: 'selected', path: '/workspace/native-project' }),
		},
		projectWorkspaceRuntime: createProjectWorkspaceRuntime(['Terminal 1']),
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const ProjectWorkspaceNavigation: Story = {
	play: ({ canvasElement }) => {
		const create = canvasElement.querySelector<HTMLButtonElement>(
			'[data-slot="workbench-sidebar-new-workspace"]',
		);
		if (create === null) throw new Error('Expected the fixed New workspace entry.');
		create.focus();
	},
	args: {
		actionRuntime: new ActionRuntime({ confirm: () => true }),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createPopulatedProjectWorkspaceRuntime(),
		shortcutPlatform: 'apple',
		terminalSurface: FocusStoryTerminalSurface,
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const ProjectWorkspaceKeyboardMenu: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createPopulatedProjectWorkspaceRuntime(),
		shortcutPlatform: 'standard',
		terminalSurface: FocusStoryTerminalSurface,
	},
	play: ({ canvasElement }) => {
		const workspace = canvasElement.querySelector<HTMLButtonElement>(
			'[data-slot="workbench-sidebar-workspace"]',
		);
		if (workspace === null) throw new Error('Expected a Workspace menu keyboard target.');
		workspace.focus();
		workspace.dispatchEvent(
			new KeyboardEvent('keydown', {
				bubbles: true,
				cancelable: true,
				code: 'F10',
				key: 'F10',
				shiftKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const ProjectWorkspaceFocusReveal: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createSidebarFocusRevealRuntime(false),
		shortcutPlatform: 'standard',
		terminalSurface: FocusStoryTerminalSurface,
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<div style={{ height: '100dvh' }}>
				<Workbench {...arguments_} />
			</div>
		</OrchbeeI18nProvider>
	),
};

export const CollapsedProjectFocusReveal: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createSidebarFocusRevealRuntime(true),
		shortcutPlatform: 'standard',
		terminalSurface: FocusStoryTerminalSurface,
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<div style={{ height: '100dvh' }}>
				<Workbench {...arguments_} />
			</div>
		</OrchbeeI18nProvider>
	),
};

const workspaceAgentOverviewStory = createWorkspaceAgentOverviewStoryFixture({ crowded: true });

export const WorkspaceAgentOverview: Story = {
	args: {
		actionRuntime: new ActionRuntime({ confirm: () => true }),
		agentObservationSource: workspaceAgentOverviewStory.agentObservationSource,
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: workspaceAgentOverviewStory.projectWorkspaceRuntime,
		shortcutPlatform: 'apple',
		terminalSurface: FocusStoryTerminalSurface,
	},
	play: ({ canvasElement }) => {
		const folds = new Set(['Hide 3 Agents in Batch Tasks', 'Hide 7 Agents in Nightly Agents']);
		for (const button of canvasElement.querySelectorAll('button')) {
			if (folds.has(button.getAttribute('aria-label') ?? '')) button.click();
		}
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

const workspaceLocationsStory = (() => {
	const fixture = createWorkspaceAgentOverviewStoryFixture();
	return {
		...fixture,
		workspaceLocationSource: createStoryWorkspaceLocationSource(fixture.projectWorkspaceRuntime, {
			'Batch Tasks': {
				folder: '~/work/clients/northwind/batch-processing/nightly-reports',
				git: null,
				heldName: null,
			},
			Docs: {
				folder: '~/work/orchbee/docs',
				git: {
					branch: 'docs/sidebar-locations-and-divergence-walkthrough',
					divergence: { ahead: 0, behind: 3 },
				},
				heldName: null,
			},
			Primary: {
				folder: '~/work/orchbee',
				git: { branch: 'main', divergence: { ahead: 2, behind: 1 } },
				heldName: null,
			},
			'Project Workspace': {
				folder: '~/work/orchbee-feature',
				git: { branch: 'feature/project-workspace', divergence: null },
				heldName: null,
			},
		}),
	};
})();

export const WorkspaceLocations: Story = {
	args: {
		actionRuntime: new ActionRuntime({ confirm: () => true }),
		agentObservationSource: workspaceLocationsStory.agentObservationSource,
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: workspaceLocationsStory.projectWorkspaceRuntime,
		shortcutPlatform: 'apple',
		terminalSurface: FocusStoryTerminalSurface,
		workspaceLocationSource: workspaceLocationsStory.workspaceLocationSource,
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

const agentStateAndUnreadFixture = (() => {
	const projectWorkspaceRuntime = createPopulatedProjectWorkspaceRuntime();
	const agents = (['working', 'blocked', 'done', 'error', 'idle', 'unknown'] as const).map(
		(state) => {
			projectWorkspaceRuntime.transition({
				directoryId: 'directory-orchbee',
				launchConfigurationId: null,
				name: state,
				resourceScopeId: 'resource-scope-1',
				projectId: 'project-1',
				terminalTitle: state,
				type: 'workspace-created',
			});
			const workspace = projectWorkspaceRuntime
				.getSnapshot()
				.workspaces.find((entry) => entry.name === state);
			if (workspace === undefined) throw new Error('Expected the status Workspace.');
			const presentationId = createStoryWorkspacePresentations(
				projectWorkspaceRuntime,
				workspace.id,
				1,
			)[0];
			return { agent: 'codex', presentationId, state };
		},
	);
	const initial = projectWorkspaceRuntime
		.getSnapshot()
		.workspaces.find((workspace) => workspace.name === 'Primary');
	if (initial === undefined) throw new Error('Expected the initial Workspace.');
	projectWorkspaceRuntime.transition({ type: 'workspace-activated', workspaceId: initial.id });
	return {
		actionRuntime: new ActionRuntime({ confirm: () => true }),
		projectWorkspaceRuntime,
		agentObservationSource: createStoryAgentSource(agents),
	};
})();

function AgentStateAndUnreadFixture() {
	const fixture = agentStateAndUnreadFixture;
	return (
		<OrchbeeI18nProvider locale='en-US'>
			<div style={{ display: 'flex', flexDirection: 'column', height: '100dvh' }}>
				<button type='button' onClick={fixture.agentObservationSource.reportAgain}>
					Publish new observations
				</button>
				<div style={{ flex: 1, minHeight: 0 }}>
					<Workbench
						{...fixture}
						shortcutPlatform='standard'
						terminalSurface={FocusStoryTerminalSurface}
					/>
				</div>
			</div>
		</OrchbeeI18nProvider>
	);
}

export const AgentStateAndUnread: Story = {
	args: { ...agentStateAndUnreadFixture, shortcutPlatform: 'standard' },
	render: () => <AgentStateAndUnreadFixture />,
};

export const SshProjectConnection: Story = {
	args: {
		actionRuntime: new ActionRuntime({ confirm: () => true }),
		environmentProjectConnection: disconnectedRemoteProject,
		onConnectViaSsh: () => undefined,
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createRemoteProjectWorkspaceRuntime(),
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const HiddenSidebar: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		layoutStorage: {
			getItem: (key) =>
				key === 'orchbee:workbench:left-panel-visibility:v1' ? '{"visible":false}' : null,
			setItem: () => undefined,
		},
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createProjectWorkspaceRuntime([
			'Terminal 1',
			'Terminal 2',
			'Terminal 3',
		]),
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const UnifiedSearchAll: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createProjectWorkspaceRuntime([
			'Terminal 1',
			'Terminal 2',
			'Terminal 3',
		]),
		shortcutPlatform: 'apple',
	},
	play: () => {
		window.dispatchEvent(
			new KeyboardEvent('keydown', {
				code: 'KeyF',
				metaKey: true,
				shiftKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const UnifiedSearchActionsChinese: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createProjectWorkspaceRuntime(['终端 1', '终端 2', '终端 3']),
		shortcutPlatform: 'apple',
	},
	play: () => {
		window.dispatchEvent(
			new KeyboardEvent('keydown', {
				code: 'KeyP',
				metaKey: true,
				shiftKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const UnifiedSearchProviderFailure: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createProjectWorkspaceRuntime([
			'Terminal 1',
			'Terminal 2',
			'Terminal 3',
		]),
		searchProviders: [unavailableSearchProvider],
		shortcutPlatform: 'apple',
	},
	play: () => {
		window.dispatchEvent(
			new KeyboardEvent('keydown', {
				code: 'KeyF',
				metaKey: true,
				shiftKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const TabTitleEditing: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createProjectWorkspaceRuntime(['Terminal 1', 'Terminal 2']),
		shortcutPlatform: 'apple',
	},
	play: () => {
		document.body.dispatchEvent(
			new KeyboardEvent('keydown', {
				bubbles: true,
				code: 'KeyI',
				metaKey: true,
				shiftKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};
