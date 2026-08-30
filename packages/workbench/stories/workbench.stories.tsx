import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { ActionRuntime } from '@orchbee/actions';
import { AgentRuntime, AgentRuntimeSnapshotSchema } from '@orchbee/agent';
import {
	Workbench,
	WorkbenchProjectWorkspaceRuntime,
	createWorkbenchPreviewTerminalPresentationComposition,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench';
import type {
	WorkbenchEnvironmentProjectConnectionCapability,
	WorkbenchProjectWorktreeCapability,
	WorkbenchSearchProvider,
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalPresentationCommand,
	WorkbenchTerminalSurfaceProps,
} from '@orchbee/workbench';
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

const meta = {
	title: 'Workbench',
	component: Workbench,
	parameters: {
		layout: 'fullscreen',
	},
} satisfies Meta<typeof Workbench>;

export default meta;

type Story = StoryObj<typeof meta>;

const unavailableSearchProvider: WorkbenchSearchProvider = {
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
		projectId: 'project-1',
		type: 'directory-added',
	});
	runtime.transition({
		directoryId: 'directory-orchbee-feature',
		launchConfigurationId: null,
		name: 'Project Workspace',
		projectId: 'project-1',
		terminalTitle: 'Terminal 1',
		type: 'workspace-created',
	});
	return runtime;
};

const storyProjectWorktreeCapability: WorkbenchProjectWorktreeCapability = {
	adopt: () => Promise.resolve({ code: 'worktree-not-found', kind: 'failed' }),
	create: () =>
		Promise.resolve({
			directoryId: 'story-created-worktree',
			kind: 'created',
			workspaceId: null,
		}),
	discover: () =>
		Promise.resolve({
			discovery: {
				managedRoot: '/Users/demo/.orchbee/worktrees',
				repositories: [
					{
						defaultBaseRef: 'main',
						label: 'orchbee',
						localBaseRefs: ['main', 'release', 'feature/dialogs'],
						repositoryId: 'orchbee',
						sourceDirectoryId: 'directory-orchbee',
						suggestedBranchName: 'feature/new-worktree',
						worktreeInventory: { kind: 'available', worktrees: [] },
					},
				],
			},
			kind: 'discovered',
		}),
};

const createWorkspaceAgentOverviewStoryFixture = () => {
	const runtime = createPopulatedProjectWorkspaceRuntime();
	runtime.transition({
		directoryId: 'directory-orchbee',
		launchConfigurationId: null,
		name: 'Batch Tasks',
		projectId: 'project-1',
		terminalTitle: 'Batch Agent 1',
		type: 'workspace-created',
	});
	runtime.transition({
		directoryId: 'directory-orchbee',
		launchConfigurationId: null,
		name: 'Docs',
		projectId: 'project-1',
		terminalTitle: 'Documentation',
		type: 'workspace-created',
	});
	const project = runtime.getSnapshot().projects[0];
	if (project === undefined) throw new Error('Expected the Storybook Project.');
	const primary = project.workspaces.find((workspace) => workspace.name === 'Primary');
	const review = project.workspaces.find((workspace) => workspace.name === 'Project Workspace');
	const batch = project.workspaces.find((workspace) => workspace.name === 'Batch Tasks');
	if (primary === undefined || review === undefined || batch === undefined) {
		throw new Error('Expected all Agent overview Storybook Workspaces.');
	}
	const primaryPresentations = createStoryWorkspacePresentations(runtime, primary.id, 1);
	const reviewPresentations = createStoryWorkspacePresentations(runtime, review.id, 2);
	const batchPresentations = createStoryWorkspacePresentations(runtime, batch.id, 3);
	runtime.transition({
		projectId: project.id,
		type: 'workspace-activated',
		workspaceId: review.id,
	});
	return {
		agentObservationSource: new AgentRuntime(
			AgentRuntimeSnapshotSchema.parse({
				revision: 1,
				sessions: [
					createStoryAgentSession({
						kind: 'working',
						presentationId: primaryPresentations[0],
						providerId: 'codex',
						sessionId: 'story-primary-codex',
					}),
					createStoryAgentSession({
						kind: 'attention',
						presentationId: reviewPresentations[0],
						providerId: 'claude-code',
						sessionId: 'story-review-claude',
					}),
					createStoryAgentSession({
						kind: 'succeeded',
						presentationId: reviewPresentations[1],
						providerId: 'codex',
						sessionId: 'story-review-codex',
					}),
					createStoryAgentSession({
						kind: 'working',
						presentationId: batchPresentations[0],
						providerId: 'codex',
						sessionId: 'story-batch-codex',
					}),
					createStoryAgentSession({
						kind: 'failed',
						presentationId: batchPresentations[1],
						providerId: 'claude-code',
						sessionId: 'story-batch-claude',
					}),
					createStoryAgentSession({
						kind: 'idle',
						presentationId: batchPresentations[2],
						providerId: 'gemini',
						sessionId: 'story-batch-gemini',
					}),
				],
			}),
		),
		projectWorkspaceRuntime: runtime,
	};
};

function createStoryWorkspacePresentations(
	runtime: WorkbenchProjectWorkspaceRuntime,
	workspaceId: string,
	count: number,
): readonly string[] {
	const project = runtime.getSnapshot().projects[0];
	const workspace = project?.workspaces.find((candidate) => candidate.id === workspaceId);
	if (project === undefined || workspace === undefined) {
		throw new Error('Expected the Storybook Workspace.');
	}
	runtime.transition({ projectId: project.id, type: 'workspace-activated', workspaceId });
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

function createStoryAgentSession(input: {
	readonly kind: 'attention' | 'failed' | 'idle' | 'succeeded' | 'working';
	readonly presentationId: string | undefined;
	readonly providerId: string;
	readonly sessionId: string;
}) {
	if (input.presentationId === undefined) {
		throw new Error('Expected a Storybook Agent presentation.');
	}
	const turnId = `turn-${input.sessionId}`;
	let capabilities = ['turnLifecycle'];
	if (input.kind === 'attention') capabilities = ['questions', 'turnLifecycle'];
	else if (input.kind === 'idle') capabilities = [];
	return {
		activityHint: null,
		capabilities,
		executionLocation: { executionLocationId: `execution-${input.sessionId}` },
		lastOutcome:
			input.kind === 'failed' || input.kind === 'succeeded'
				? {
						eventId: `outcome-${input.sessionId}`,
						kind: input.kind,
						turnId,
					}
				: null,
		lifecycle: 'live',
		observation: {
			fidelity: 'rich',
			freshness: 'fresh',
			generation: 0,
			sequence: 1,
			source: 'hook',
		},
		parentSessionId: null,
		pendingRequests:
			input.kind === 'attention'
				? [
						{
							actionPath: 'navigate-only',
							kind: 'question',
							requestId: `request-${input.sessionId}`,
							turnId,
						},
					]
				: [],
		presentationAttachments: [
			{
				generation: `attachment-${input.sessionId}`,
				navigable: true,
				presentationId: input.presentationId,
				visibility: 'visible',
			},
		],
		providerId: input.providerId,
		sessionId: input.sessionId,
		turn:
			input.kind === 'attention' || input.kind === 'working'
				? { activity: 'editing', kind: 'active', phase: 'working', turnId }
				: { kind: 'idle' },
	};
}

const createSshFolderManagementRuntime = () => {
	const runtime = createPopulatedProjectWorkspaceRuntime();
	runtime.transition({
		directory: {
			available: false,
			git: null,
			id: 'environment-directory:44444444444444444444444444444444',
			kind: 'folder',
			name: 'remote-services',
			path: '/srv/orchbee/services',
			placement: {
				environmentId: 'environment:00000000000000000000000000000001',
				kind: 'environment',
			},
			source: 'selected',
		},
		projectId: 'project-1',
		type: 'environment-directory-registered',
	});
	return runtime;
};

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
	presentation,
	presentationMode,
}: WorkbenchTerminalSurfaceProps) {
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
			data-slot='storybook-terminal-input'
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
			value={`${presentation.title}\n$ `}
			readOnly
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
	args: {
		actionRuntime: new ActionRuntime({ confirm: () => true }),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createPopulatedProjectWorkspaceRuntime(),
		projectWorktree: storyProjectWorktreeCapability,
		shortcutPlatform: 'apple',
		terminalSurface: FocusStoryTerminalSurface,
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

const workspaceAgentOverviewStory = createWorkspaceAgentOverviewStoryFixture();

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
		const disclosure = [...canvasElement.querySelectorAll('button')].find(
			(button) => button.getAttribute('aria-label') === 'Show 2 Agents in Project Workspace',
		);
		disclosure?.click();
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
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

export const NativeProjectFolderManagement: Story = {
	args: {
		actionRuntime: new ActionRuntime({ confirm: () => true }),
		onAddProjectFolderViaSsh: () => undefined,
		onSettings: () => ({ kind: 'unchanged' }),
		projectFolderAcquisition: {
			selectExistingFolder: () =>
				Promise.resolve({ kind: 'selected', path: '/workspace/native-added' }),
		},
		projectWorkspaceRuntime: createSshFolderManagementRuntime(),
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
