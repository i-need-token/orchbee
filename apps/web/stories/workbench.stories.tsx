import type { ComponentProps } from 'react';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { ActionRuntime } from '@orchbee/actions';
import { Workbench } from '../src/workbench';
import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAuthority,
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalSurfaceProps,
	WorkbenchWorkspaceLocation,
	WorkbenchWorkspaceLocationSource,
} from '../src/workbench';
import {
	createSnapshotRuntime,
	fixtureDirectoryId,
	fixturePresentationId,
	fixtureProjectId,
	fixtureWorkspaceId,
	snapshotContext,
	snapshotDirectory,
	snapshotPanel,
	snapshotPresentation,
	snapshotProject,
	snapshotSplit,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
	type FixtureRecord,
} from '../src/__tests__/server-workbench-snapshot';
import { useEffect, useLayoutEffect, useRef } from 'react';
import type { Meta, StoryObj } from 'storybook-react-rsbuild';

/** Decides nothing: each Story shows a fixed Server snapshot, so every write leaves it as it is. */
const staticStoryAuthority: WorkbenchAuthority = {
	change: () => Promise.resolve({ kind: 'unchanged' }),
	close: () => Promise.resolve({ kind: 'unchanged' }),
	create: () => Promise.resolve({ kind: 'unsupported' }),
	createPanelTerminal: () => Promise.resolve({ kind: 'unsupported' }),
	createTab: () => Promise.resolve({ kind: 'unsupported' }),
	duplicateTabs: () => Promise.resolve({ kind: 'unsupported' }),
	minimumSplitPercent: 10,
	watchClose: () => () => undefined,
};

const meta = {
	title: 'Workbench',
	component: Workbench,
	args: {
		authority: staticStoryAuthority,
	},
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

const ORCHBEE_FOLDER = '/workspace/orchbee';
const ORCHBEE_FEATURE_FOLDER = '/workspace/orchbee-feature';

interface StoryWorkspace {
	/** The Workspace the window shows; the last one marked wins. */
	readonly active?: boolean;
	/** Workspaces that name the same folder share one directory. */
	readonly folder: string;
	readonly name: string;
	/** One Tab per title, each holding one terminal of that name. */
	readonly titles: readonly string[];
}

interface StoryProject {
	readonly collapsed?: boolean;
	readonly name: string;
	readonly workspaces: readonly StoryWorkspace[];
}

interface StoryWorkbench {
	/** Each Workspace's terminals in Tab order, by Workspace name. */
	readonly presentationIds: ReadonlyMap<string, readonly string[]>;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}

const titledPresentation = (sequence: number, title: string) =>
	snapshotPresentation(sequence, { customLabel: title });

const numberedTitles = (name: string, count: number): readonly string[] =>
	Array.from({ length: count }, (_, index) => `${name} ${index + 1}`);

/** The window a Server publishes these Projects and Workspaces to, showing the active Workspace. */
function createStoryWorkbench({
	projects = [],
	workspaces = [],
}: {
	readonly projects?: readonly StoryProject[];
	readonly workspaces?: readonly StoryWorkspace[];
}): StoryWorkbench {
	const folders = new Map<string, number>();
	const directories: FixtureRecord[] = [];
	const workspaceRecords: FixtureRecord[] = [];
	const contexts: FixtureRecord[] = [];
	const presentationIds = new Map<string, readonly string[]>();
	let activeWorkspaceId: string | null = null;
	let tabSequence = 0;
	const addWorkspace = (workspace: StoryWorkspace, projectId: string | null): void => {
		let directory = folders.get(workspace.folder);
		if (directory === undefined) {
			directory = folders.size + 1;
			folders.set(workspace.folder, directory);
			directories.push({ ...snapshotDirectory(directory), path: workspace.folder });
		}
		const sequence = workspaceRecords.length + 1;
		workspaceRecords.push(
			snapshotWorkspace(sequence, {
				label: workspace.name,
				primaryDirectoryId: fixtureDirectoryId(directory),
				projectId,
			}),
		);
		const tabs = workspace.titles.map((title) => ({ sequence: (tabSequence += 1), title }));
		contexts.push(
			snapshotContext(
				sequence,
				tabs.map((tab) =>
					snapshotTab(
						tab.sequence,
						snapshotPanel(tab.sequence, titledPresentation(tab.sequence, tab.title)),
						{
							label: tab.title,
						},
					),
				),
			),
		);
		presentationIds.set(
			workspace.name,
			tabs.map((tab) => fixturePresentationId(tab.sequence)),
		);
		if (workspace.active === true) activeWorkspaceId = fixtureWorkspaceId(sequence);
	};
	for (const [index, project] of projects.entries()) {
		for (const workspace of project.workspaces)
			addWorkspace(workspace, fixtureProjectId(index + 1));
	}
	for (const workspace of workspaces) addWorkspace(workspace, null);
	const runtime = createSnapshotRuntime(
		workbenchSnapshot(
			1,
			{
				directories,
				projects: projects.map((project, index) => snapshotProject(index + 1, project.name)),
				workspaces: workspaceRecords,
			},
			contexts,
		),
		{
			expandedProjectIds: projects.flatMap((project, index) =>
				project.collapsed === true ? [] : [fixtureProjectId(index + 1)],
			),
		},
	);
	if (activeWorkspaceId !== null) {
		runtime.transition({ type: 'workspace-activated', workspaceId: activeWorkspaceId });
	}
	return { presentationIds, runtime };
}

/** One Workspace outside any Project, holding one Tab per title. */
const createTerminalTabsRuntime = (titles: readonly string[]) =>
	createStoryWorkbench({
		workspaces: [{ active: true, folder: ORCHBEE_FOLDER, name: 'orchbee', titles }],
	}).runtime;

/**
 * The orchbee Project: its Primary Workspace, and the Project Workspace on a feature worktree,
 * which the window shows unless `primaryActive`.
 */
const orchbeeProject = (
	extraWorkspaces: readonly StoryWorkspace[] = [],
	{
		primaryActive = false,
		primaryTitles = ['Terminal 1'],
		reviewTitles = ['Terminal 1'],
	}: {
		readonly primaryActive?: boolean;
		readonly primaryTitles?: readonly string[];
		readonly reviewTitles?: readonly string[];
	} = {},
): StoryProject => ({
	name: 'orchbee',
	workspaces: [
		{ active: primaryActive, folder: ORCHBEE_FOLDER, name: 'Primary', titles: primaryTitles },
		{
			active: !primaryActive,
			folder: ORCHBEE_FEATURE_FOLDER,
			name: 'Project Workspace',
			titles: reviewTitles,
		},
		...extraWorkspaces,
	],
});

const createPopulatedProjectWorkspaceRuntime = () =>
	createStoryWorkbench({ projects: [orchbeeProject()] }).runtime;

/** Every drag destination: two listed Projects, a folded one, and a Workspace outside any Project. */
const createDragProjectWorkspaceRuntime = () =>
	createStoryWorkbench({
		projects: [
			orchbeeProject(),
			{
				collapsed: true,
				name: 'orchbee-design',
				workspaces: [
					{ folder: '/workspace/orchbee-design', name: 'Brand', titles: ['Terminal 1'] },
				],
			},
			{
				name: 'tui-design-kit',
				workspaces: [{ folder: '/workspace/tui-design-kit', name: 'Kit', titles: ['Terminal 1'] }],
			},
		],
		workspaces: [{ folder: '/workspace/scratch', name: 'Scratch', titles: ['Terminal 1'] }],
	}).runtime;

const createSidebarFocusRevealRuntime = (collapsed: boolean) =>
	createStoryWorkbench({
		projects: [
			orchbeeProject(),
			...Array.from({ length: 24 }, (_, index): StoryProject => {
				const sequence = index + 1;
				return {
					collapsed: collapsed && sequence === 24,
					name: `Focus Project ${sequence}`,
					workspaces: [
						{
							active: true,
							folder: `/workspace/focus-${sequence}`,
							name: 'Primary',
							titles: [`Focus Terminal ${sequence}`],
						},
					],
				};
			}),
		],
	}).runtime;

/**
 * The crowded Workspace's Agents, more than a folded status line holds at the default width,
 * so it shows the states that fit and a `+N` for the rest.
 */
const CROWDED_AGENT_STATES = [
	'working',
	'idle',
	'blocked',
	'working',
	'unknown',
	'done',
	'error',
	'idle',
	'working',
	'done',
	'idle',
	'blocked',
	'working',
	'unknown',
	'done',
	'error',
	'idle',
	'blocked',
	'working',
	'done',
] as const;

/** `crowded` adds a Workspace holding more Agents than a folded status line shows. */
const createWorkspaceAgentOverviewStoryFixture = ({ crowded = false } = {}) => {
	const { presentationIds, runtime } = createStoryWorkbench({
		projects: [
			orchbeeProject(
				[
					{ folder: ORCHBEE_FOLDER, name: 'Batch Tasks', titles: numberedTitles('Batch Tasks', 3) },
					{ folder: ORCHBEE_FOLDER, name: 'Docs', titles: ['Documentation'] },
					...(crowded
						? [
								{
									folder: ORCHBEE_FOLDER,
									name: 'Nightly Agents',
									titles: numberedTitles('Nightly Agents', CROWDED_AGENT_STATES.length),
								},
							]
						: []),
				],
				{
					primaryTitles: numberedTitles('Primary', 1),
					reviewTitles: numberedTitles('Project Workspace', 2),
				},
			),
		],
	});
	const primary = presentationIds.get('Primary') ?? [];
	const review = presentationIds.get('Project Workspace') ?? [];
	const batch = presentationIds.get('Batch Tasks') ?? [];
	const nightly = presentationIds.get('Nightly Agents') ?? [];
	return {
		agentObservationSource: createStoryAgentSource([
			{ agent: 'codex', presentationId: primary[0], state: 'working' },
			{ agent: 'claude', presentationId: review[0], state: 'blocked' },
			{ agent: 'codex', presentationId: review[1], state: 'done' },
			{ agent: 'codex', presentationId: batch[0], state: 'working' },
			{ agent: 'claude', presentationId: batch[1], state: 'error' },
			{ agent: 'gemini', presentationId: batch[2], state: 'idle' },
			...nightly.map((presentationId, index) => ({
				agent: index % 2 === 0 ? 'codex' : 'claude',
				presentationId,
				state: CROWDED_AGENT_STATES[index] ?? 'unknown',
			})),
		]),
		projectWorkspaceRuntime: runtime,
	};
};

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

/** One Tab of three side-by-side Panels, as two splits to the right leave it. */
const createThreePanelRuntime = () => {
	const panel = (sequence: number) =>
		snapshotPanel(sequence, titledPresentation(sequence, `Terminal ${sequence}`));
	const root = snapshotSplit(
		1,
		'horizontal',
		panel(1),
		snapshotSplit(2, 'horizontal', panel(2), panel(3)),
		1 / 3,
	);
	const runtime = createSnapshotRuntime(
		workbenchSnapshot(
			1,
			{
				directories: [{ ...snapshotDirectory(1), path: ORCHBEE_FOLDER }],
				workspaces: [snapshotWorkspace(1, { label: 'orchbee' })],
			},
			[snapshotContext(1, [snapshotTab(1, root, { label: 'Terminal 1' })])],
		),
	);
	runtime.transition({ type: 'workspace-activated', workspaceId: fixtureWorkspaceId(1) });
	return runtime;
};

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

export const PanelWorkspace: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createTerminalTabsRuntime(['Terminal 1', 'Terminal 2', 'Terminal 3']),
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

/** Inspect Rename and its blank-name help through the same Panel menu in Chinese. */
export const PanelWorkspaceChinese: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createTerminalTabsRuntime(['Review', 'Build', 'Shell']),
		shortcutPlatform: 'standard',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

export const PanelVisualSelection: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createThreePanelRuntime(),
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
		projectWorkspaceRuntime: createThreePanelRuntime(),
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

export const SimplifiedChineseSidebar: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createTerminalTabsRuntime(['终端 1', '终端 2', '终端 3']),
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

/** A development build's sidebar footer: the environment word and checkout name beside Settings. */
export const DevelopmentEnvironmentMarker: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		environmentMarker: { checkoutName: 'orchbee', environment: 'development' },
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createTerminalTabsRuntime(['Terminal 1']),
		shortcutPlatform: 'standard',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
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
		projectWorkspaceRuntime: createTerminalTabsRuntime(['Terminal 1']),
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

export const ProjectWorkspaceDrag: Story = {
	args: {
		actionRuntime: new ActionRuntime({ confirm: () => true }),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createDragProjectWorkspaceRuntime(),
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
		const folds = new Set([
			'Hide 3 Agents in Batch Tasks',
			`Hide ${CROWDED_AGENT_STATES.length} Agents in Nightly Agents`,
		]);
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
	const states = ['working', 'blocked', 'done', 'error', 'idle', 'unknown'] as const;
	const { presentationIds, runtime } = createStoryWorkbench({
		projects: [
			orchbeeProject(
				states.map((state) => ({
					folder: ORCHBEE_FOLDER,
					name: state,
					titles: numberedTitles(state, 1),
				})),
				{ primaryActive: true },
			),
		],
	});
	return {
		actionRuntime: new ActionRuntime({ confirm: () => true }),
		agentObservationSource: createStoryAgentSource(
			states.map((state) => ({
				agent: 'codex',
				presentationId: presentationIds.get(state)?.[0],
				state,
			})),
		),
		authority: staticStoryAuthority,
		projectWorkspaceRuntime: runtime,
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

export const HiddenSidebar: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		layoutStorage: {
			getItem: (key) =>
				key === 'orchbee:workbench:left-panel-visibility:v1' ? '{"visible":false}' : null,
			setItem: () => undefined,
		},
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createTerminalTabsRuntime(['Terminal 1', 'Terminal 2', 'Terminal 3']),
		shortcutPlatform: 'apple',
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};

/**
 * Below 481 px wide (ORC-342): the bar in place of the Tab strip, whose `Tabs` button and middle
 * open the Tab list, the sidebar filling the window, and touch sizes from 408 px high.
 */
export const MobileLayout: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createTerminalTabsRuntime(['Terminal 1', 'Terminal 2', 'Terminal 3']),
		shortcutPlatform: 'standard',
	},
	globals: { viewport: { isRotated: false, value: 'mobile2' } },
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<div style={{ height: '100dvh' }}>
				<Workbench {...arguments_} />
			</div>
		</OrchbeeI18nProvider>
	),
};

/** The mobile layout in Chinese, whose wider `标签页 8` leaves the current Tab less of the bar. */
export const MobileLayoutChinese: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createTerminalTabsRuntime(
			Array.from({ length: 8 }, (_, index) => `终端 ${index + 1}`),
		),
		shortcutPlatform: 'standard',
	},
	globals: { viewport: { isRotated: false, value: 'mobile2' } },
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='zh-CN'>
			<div style={{ height: '100dvh' }}>
				<Workbench {...arguments_} />
			</div>
		</OrchbeeI18nProvider>
	),
};

/** The window while the Server holds no Workspace. */
export const NoWorkspace: Story = {
	args: {
		actionRuntime: new ActionRuntime(),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createStoryWorkbench({}).runtime,
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
		projectWorkspaceRuntime: createTerminalTabsRuntime(['Terminal 1', 'Terminal 2', 'Terminal 3']),
		searchOffered: true,
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
		projectWorkspaceRuntime: createTerminalTabsRuntime(['终端 1', '终端 2', '终端 3']),
		searchOffered: true,
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
		projectWorkspaceRuntime: createTerminalTabsRuntime(['Terminal 1', 'Terminal 2', 'Terminal 3']),
		searchOffered: true,
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
		projectWorkspaceRuntime: createTerminalTabsRuntime(['Terminal 1', 'Terminal 2']),
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
