import { useMemo, useState, useSyncExternalStore, type ReactNode } from 'react';
import {
	TabModel,
	GlicControlsController,
	ProjectsPanelController,
	TabStripOrientation,
	ThumbnailCaptureReadiness,
	type TabData,
	type TabGroupData,
	type HoverCardThumbnailSource,
	type HoverCardThumbnailSubscription,
	type TabAlertState,
} from '../../src';
import { GlicControls, ProjectsPanel, TabStrip } from '../../src/react';

const initialGroups: TabGroupData[] = [
	{
		id: 'research',
		title: 'Research',
		color: 'blue',
		collapsed: false,
		saved: true,
		shared: true,
		attention: true,
		members: [
			{ id: 'alex', displayName: 'Alex' },
			{ id: 'sam', displayName: 'Sam' },
			{ id: 'taylor', displayName: 'Taylor' },
			{ id: 'morgan', displayName: 'Morgan' },
			{ id: 'jamie', displayName: 'Jamie' },
		],
		recentActivity: [
			{
				id: 'tab-update',
				scope: 'tab',
				title: 'Alex updated the tab-strip sources',
				description: 'Alex',
				timeText: '2m',
				action: 'focus-tab',
				tabId: 'source',
			},
			{
				id: 'member-add',
				scope: 'group',
				title: 'Sam joined the group',
				description: 'Sam',
				timeText: '8m',
				action: 'manage-sharing',
			},
			{
				id: 'group-rename',
				scope: 'group',
				title: 'Group renamed to Research',
				description: 'Taylor',
				timeText: '1h',
				action: 'edit-group',
			},
		],
	},
	{ id: 'build', title: 'Build', color: 'purple', collapsed: false },
];

const sourceThumbnail =
	'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 360"%3E%3Cdefs%3E%3ClinearGradient id="g" x2="1" y2="1"%3E%3Cstop stop-color="%234285f4"/%3E%3Cstop offset="1" stop-color="%236f42e8"/%3E%3C/linearGradient%3E%3C/defs%3E%3Crect width="640" height="360" fill="url(%23g)"/%3E%3Crect x="48" y="48" width="544" height="264" rx="18" fill="%23fff" fill-opacity=".92"/%3E%3Ccircle cx="92" cy="88" r="9" fill="%23ea4335"/%3E%3Ccircle cx="120" cy="88" r="9" fill="%23fbbc04"/%3E%3Ccircle cx="148" cy="88" r="9" fill="%2334a853"/%3E%3Cpath d="M96 154h310M96 194h448M96 234h380" stroke="%239aa0a6" stroke-width="16" stroke-linecap="round"/%3E%3C/svg%3E';

class FixtureThumbnailSource implements HoverCardThumbnailSource<string> {
	private callback: ((image: string) => void) | null = null;

	public constructor(private readonly image: string) {}
	public hasData() {
		return true;
	}
	public getCaptureReadiness() {
		return ThumbnailCaptureReadiness.ReadyForFinalCapture;
	}
	public subscribe(): HoverCardThumbnailSubscription<string> {
		return {
			setSizeHint: () => {},
			setImageCallback: (callback) => {
				this.callback = callback;
			},
			dispose: () => {
				this.callback = null;
			},
		};
	}
	public requestThumbnailImage() {
		window.setTimeout(() => this.callback?.(this.image), 40);
	}
}

const initialTabs: TabData[] = [
	{
		id: 'mail',
		title: 'Inbox (3) – Example Mail',
		url: 'https://mail.example.test',
		pinned: true,
		group: null,
		split: null,
		alertState: 'none',
	},
	{
		id: 'calendar',
		title: 'Calendar',
		url: 'https://calendar.example.test',
		pinned: true,
		group: null,
		split: null,
		alertState: 'none',
	},
	{
		id: 'source',
		title: 'orchbee / orchbee',
		url: 'https://source.example.test/orchbee',
		thumbnailUrl: sourceThumbnail,
		memoryUsageBytes: 184 * 1024 * 1024,
		pinned: false,
		group: 'research',
		split: null,
		alertState: 'none',
	},
	{
		id: 'tabs-source',
		title: 'packages/tab-strip',
		url: 'https://source.example.test/packages/tab-strip',
		pinned: false,
		group: 'research',
		split: null,
		alertState: 'none',
	},
	{
		id: 'docs',
		title: 'Tab strip architecture notes',
		url: 'https://docs.example.test/tab-strip',
		pinned: false,
		group: 'build',
		split: 'split-build',
		alertState: 'none',
	},
	{
		id: 'preview',
		title: 'React component preview',
		url: 'http://localhost:5173',
		pinned: false,
		group: 'build',
		split: 'split-build',
		alertState: 'audio-playing',
	},
	{
		id: 'issue',
		title: 'Drag-and-drop interaction audit',
		url: 'https://issues.example.test/142',
		pinned: false,
		group: null,
		split: null,
		loading: true,
		discarded: true,
		memoryUsageBytes: 42 * 1024 * 1024,
		alertState: 'none',
	},
	{
		id: 'new-tab',
		title: 'New Tab',
		url: 'chrome://newtab',
		pinned: false,
		group: null,
		split: null,
		alertState: 'none',
	},
];

const alertGalleryStates: readonly TabAlertState[] = [
	'bluetooth-connected',
	'actor-accessing',
	'video-recording',
	'desktop-capturing',
	'tab-capturing',
	'pip-playing',
	'glic-sharing',
	'usb-connected',
];

export function SystemTestFixture(): ReactNode {
	const model = useMemo(
		() =>
			new TabModel({
				tabs: new URLSearchParams(window.location.search).has('alerts')
					? initialTabs.map((tab, index) => ({
							...tab,
							alertState: alertGalleryStates[index] ?? 'none',
						}))
					: initialTabs,
				groups: initialGroups,
				activeTabId: 'tabs-source',
			}),
		[],
	);
	const snapshot = useSyncExternalStore(model.subscribe, model.getSnapshot, model.getSnapshot);
	const [orientation, setOrientation] = useState(TabStripOrientation.Horizontal);
	const [collapsed, setCollapsed] = useState(false);
	const [expandOnHover, setExpandOnHover] = useState(true);
	const [projectsOpen, setProjectsOpen] = useState(false);
	const [tabSearchOpen, setTabSearchOpen] = useState(false);
	const thumbnailSources = useMemo(
		() => new Map([['source', new FixtureThumbnailSource(sourceThumbnail)]]),
		[],
	);
	const glicController = useMemo(() => new GlicControlsController({ actorVisible: true }), []);
	const projectsController = useMemo(() => {
		let controller: ProjectsPanelController;
		const reorder = (groupId: string, targetId: string, before: boolean) => {
			const groups = [...controller.getSnapshot().tabGroups];
			const moving = groups.find((group) => group.id === groupId);
			if (!moving) return;
			const remaining = groups.filter((group) => group.id !== groupId);
			const targetIndex = remaining.findIndex((group) => group.id === targetId);
			remaining.splice(Math.max(0, targetIndex + (before ? 0 : 1)), 0, moving);
			controller.reorderTabGroups(remaining);
		};
		controller = new ProjectsPanelController({
			onOpenTabGroup: (groupId) => {
				const first = model.getSnapshot().tabs.find((tab) => tab.group === groupId);
				if (first) model.selectTab(first.id);
				setProjectsOpen(false);
			},
			onReorderGroupBefore: (groupId, targetId) => reorder(groupId, targetId, true),
			onReorderGroupAfter: (groupId, targetId) => reorder(groupId, targetId, false),
			onOpenThread: (thread) => {
				model.addTab({
					title: thread.title,
					url: `https://gemini.google.com/app/${thread.serverId}`,
					pinned: false,
					group: null,
					split: null,
				});
				setProjectsOpen(false);
			},
		});
		controller.initializeTabGroups([
			{ id: 'research', title: 'Research', color: 'blue', tabCount: 2 },
			{ id: 'build', title: 'Build', color: 'purple', tabCount: 2 },
		]);
		controller.initializeThreads([
			{
				taskId: 'task-1',
				serverId: 'tab-strip-review',
				title: 'Review the tab strip port',
				lastTurnTime: 4000,
				type: 'gemini',
			},
			{
				taskId: 'task-2',
				serverId: 'dragging-audit',
				title: 'Audit dragging edge cases',
				lastTurnTime: 3000,
				type: 'ai-mode',
			},
			{
				taskId: 'task-3',
				serverId: 'vertical-tabs',
				title: 'Compare vertical tab layouts',
				lastTurnTime: 2000,
				type: 'gemini',
			},
			{
				taskId: 'task-4',
				serverId: 'hover-cards',
				title: 'Summarize hover card behavior',
				lastTurnTime: 1000,
				type: 'ai-mode',
			},
		]);
		return controller;
	}, [model]);
	const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId);

	return (
		<div className='fixture-shell'>
			<header className='fixture-controls'>
				<div>
					<strong>Tab Strip React</strong>
					<span>{snapshot.tabs.length} tabs · source-port preview</span>
				</div>
				<div className='fixture-segmented' aria-label='Tab strip orientation'>
					<button
						data-active={orientation === TabStripOrientation.Horizontal || undefined}
						onClick={() => {
							setOrientation(TabStripOrientation.Horizontal);
							setTabSearchOpen(false);
						}}
					>
						Horizontal
					</button>
					<button
						data-active={orientation === TabStripOrientation.Vertical || undefined}
						onClick={() => setOrientation(TabStripOrientation.Vertical)}
					>
						Vertical
					</button>
				</div>
				<button
					className='fixture-collapse'
					aria-expanded={projectsOpen}
					onClick={() => setProjectsOpen((value) => !value)}
				>
					Projects
				</button>
				<GlicControls
					controller={glicController}
					activeTabId={snapshot.activeTabId}
					onActorClick={() => glicController.triggerActorNudge('1 task running')}
				/>
			</header>

			<main
				className='fixture-browser'
				data-orientation={orientation}
				data-collapsed={collapsed || undefined}
			>
				<ProjectsPanel
					controller={projectsController}
					open={projectsOpen}
					onClose={() => setProjectsOpen(false)}
					targetWidth={orientation === TabStripOrientation.Vertical && !collapsed ? 240 : 300}
				/>
				{tabSearchOpen ? (
					<aside className='fixture-tab-search' role='dialog' aria-label='Search tabs'>
						<strong>Search tabs</strong>
						<span>{snapshot.tabs.length} open tabs</span>
						<div>
							{snapshot.tabs.map((tab) => (
								<button
									key={tab.id}
									onClick={() => {
										model.selectTab(tab.id);
										setTabSearchOpen(false);
									}}
								>
									<span>{tab.title.slice(0, 1).toUpperCase()}</span>
									{tab.title}
								</button>
							))}
						</div>
					</aside>
				) : null}
				<TabStrip
					model={model}
					orientation={orientation}
					verticalCollapsed={collapsed}
					verticalExpandOnHover={expandOnHover}
					showVerticalProjectsButton
					showVerticalTabSearchButton
					verticalProjectsOpen={projectsOpen}
					verticalTabSearchOpen={tabSearchOpen}
					onVerticalCollapsedChange={setCollapsed}
					onVerticalProjectsClick={() => {
						setTabSearchOpen(false);
						setProjectsOpen((value) => !value);
					}}
					onVerticalTabSearchClick={() => {
						setProjectsOpen(false);
						setTabSearchOpen((value) => !value);
					}}
					onVerticalExpandOnHoverChange={setExpandOnHover}
					getHoverCardThumbnailSource={(tab) => thumbnailSources.get(tab.id) ?? null}
				/>
				<section className='fixture-page'>
					<div className='fixture-toolbar'>
						<button aria-label='Back'>←</button>
						<button aria-label='Forward'>→</button>
						<button aria-label='Reload'>↻</button>
						<div className='fixture-address'>
							<span>⌾</span>
							{activeTab?.url ?? 'chrome://newtab'}
						</div>
						<button aria-label='Bookmark'>☆</button>
						<button aria-label='Application menu'>⋮</button>
					</div>
					<div className='fixture-content'>
						<div className='fixture-content__mark'>
							{activeTab?.title.slice(0, 1).toUpperCase() ?? '+'}
						</div>
						<h1>{activeTab?.title ?? 'New Tab'}</h1>
						<p>{activeTab?.url ?? 'chrome://newtab'}</p>
						<div className='fixture-hints'>
							<span>Drag tabs to reorder</span>
							<span>Ctrl/⌘ click for multi-select</span>
							<span>Right-click tabs and groups</span>
							<span>Middle-click to close</span>
						</div>
					</div>
				</section>
			</main>
		</div>
	);
}
