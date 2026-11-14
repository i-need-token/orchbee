import { useMemo, useState, useSyncExternalStore, type ReactNode } from 'react';
import { TabModel, TabStripOrientation, TabStrip, type TabData } from '../../src';
import { ThumbnailCaptureReadiness } from '../../src/tab-strip/components/tab-strip-view/components/hover-card/model/tab-hover-card-controller';
import type {
	HoverCardThumbnailSource,
	HoverCardThumbnailSubscription,
} from '../../src/tab-strip/components/tab-strip-view/components/hover-card/model/TabHoverCardThumbnailObserver';
import type { TabAlertState, TabGroupData } from '../../src/tab-strip/model/tab-model';

const initialGroups: TabGroupData[] = [
	{
		id: 'research',
		title: 'Research',
		color: 'blue',
		collapsed: false,
		attention: true,
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
		alertState: 'none',
	},
	{
		id: 'calendar',
		title: 'Calendar',
		url: 'https://calendar.example.test',
		pinned: true,
		group: null,
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
		alertState: 'none',
	},
	{
		id: 'tabs-source',
		title: 'packages/tab-strip',
		url: 'https://source.example.test/packages/tab-strip',
		pinned: false,
		group: 'research',
		alertState: 'none',
	},
	{
		id: 'docs',
		title: 'Tab strip architecture notes',
		url: 'https://docs.example.test/tab-strip',
		pinned: false,
		group: 'build',
		alertState: 'none',
	},
	{
		id: 'preview',
		title: 'React component preview',
		url: 'http://localhost:5173',
		pinned: false,
		group: 'build',
		alertState: 'audio-playing',
	},
	{
		id: 'issue',
		title: 'Drag-and-drop interaction audit',
		url: 'https://issues.example.test/142',
		pinned: false,
		group: null,
		loading: true,
		discarded: true,
		memoryUsageBytes: 42 * 1024 * 1024,
		alertState: 'none',
	},
	{
		id: 'new-tab',
		title: 'New Tab',
		pinned: false,
		group: null,
		alertState: 'none',
	},
];

const alertGalleryStates: readonly TabAlertState[] = [
	'bluetooth-connected',
	'audio-recording',
	'video-recording',
	'desktop-capturing',
	'tab-capturing',
	'pip-playing',
	'serial-connected',
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
	const thumbnailSources = useMemo(
		() => new Map([['source', new FixtureThumbnailSource(sourceThumbnail)]]),
		[],
	);
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
						onClick={() => setOrientation(TabStripOrientation.Horizontal)}
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
			</header>

			<main className='fixture-surface' data-orientation={orientation}>
				<TabStrip
					model={model}
					orientation={orientation}
					// A product host asks for the name; the fixture leaves every group it names unnamed.
					onNameGroup={(request) => request.commit('')}
					getHoverCardThumbnailSource={(tab) => thumbnailSources.get(tab.id) ?? null}
				/>
				<section className='fixture-page'>
					<div className='fixture-content'>
						<div className='fixture-content__mark'>
							{activeTab?.title.slice(0, 1).toUpperCase() ?? '+'}
						</div>
						<h1>{activeTab?.title ?? 'New Tab'}</h1>
						<p>{activeTab?.url ?? 'New tab'}</p>
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
