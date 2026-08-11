import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { ActionRuntime } from '@orchbee/actions';
import {
	Workbench,
	WorkbenchProjectWorkspaceRuntime,
	createWorkbenchPreviewTerminalPresentationComposition,
	terminalPresentationId,
} from '@orchbee/workbench';
import type { WorkbenchSearchProvider } from '@orchbee/workbench';
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

export const ProjectWorkspaceNavigation: Story = {
	args: {
		actionRuntime: new ActionRuntime({ confirm: () => true }),
		onSettings: () => ({ kind: 'unchanged' }),
		projectWorkspaceRuntime: createPopulatedProjectWorkspaceRuntime(),
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
				code: 'KeyK',
				metaKey: true,
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
				code: 'KeyK',
				metaKey: true,
			}),
		);
	},
	render: (arguments_) => (
		<OrchbeeI18nProvider locale='en-US'>
			<Workbench {...arguments_} />
		</OrchbeeI18nProvider>
	),
};
