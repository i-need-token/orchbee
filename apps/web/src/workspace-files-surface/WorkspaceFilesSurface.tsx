import { useMemoizedFn } from '@orchbee/foundation/react';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { TabModel } from '@orchbee/tab-strip/model';
import * as React from 'react';
import { createPortal } from 'react-dom';

import type { WorkbenchAgentSource, WorkbenchWorkspaceFilesSource } from '../workbench';
import type { DiffOptions } from './components/float-window/components/editor-pane/EditorPane';
import { FloatWindow } from './components/float-window/FloatWindow';
import { EditorDocuments } from './EditorDocuments';
import { floatOf, reduceFloatWindows } from './float-reducer';
import { documentPathOf, type FloatTarget } from './float-tabs';
import { type AutoRefresh, RightBar } from './components/right-bar/RightBar';
import { useAgentSettled } from './hooks/useAgentSettled';
import { type WorkspaceFilesChange, useWorkspaceFilesWatch } from './hooks/useWorkspaceFilesWatch';

export interface WorkspaceFilesSurfaceProps {
	readonly source: WorkbenchWorkspaceFilesSource;
	/** The terminal Agents; one that stops working refreshes what shows. */
	readonly agentSource?: WorkbenchAgentSource | undefined;
	/** The Workspace whose files show; null shows a note in the bar and no float window. */
	readonly workspaceId: string | null;
	/** Where the right bar draws; null while it is closed. */
	readonly barHost: HTMLElement | null;
	/** Where the float window draws: the Tab content area. */
	readonly floatHost: HTMLElement | null;
	/** The current Tabs; choosing another one collapses the float window. */
	readonly tabModel: TabModel | null;
	readonly onCloseBar: () => void;
	readonly closeShortcutLabel?: string | undefined;
	/** The platform the float window's own shortcuts match and show for. */
	readonly shortcutPlatform: KeyboardShortcutPlatform;
	/** Hands the keyboard back to the terminal when the float window collapses or closes. */
	readonly onReturnFocus: () => void;
}

/** VS Code's defaults: side by side where it fits, and leading or trailing whitespace ignored. */
const DEFAULT_DIFF_OPTIONS: DiffOptions = {
	hideUnchangedRegions: false,
	ignoreTrimWhitespace: true,
	sideBySide: true,
};

const NO_TABS = (): (() => void) => () => {};
const NO_ACTIVE_TAB = (): null => null;
const NO_FOLDERS: ReadonlySet<string> = new Set();
const EVERYTHING: WorkspaceFilesChange = { files: true, repository: true };

/** Shallower folders first, so a watch that cannot take them all keeps the ones nearest the top. */
function byDepth(left: string, right: string): number {
	const depth = (path: string): number => path.split('/').length;
	const difference = depth(left) - depth(right);
	if (difference !== 0) return difference;
	return left < right ? -1 : Number(left > right);
}

/**
 * What the right bar brings to a window: the bar itself and the float window that opens its files.
 * It stays mounted once loaded, so the float window and its files outlive closing the bar, and each
 * Workspace keeps its own float window.
 */
export function WorkspaceFilesSurface({
	source,
	agentSource,
	workspaceId,
	barHost,
	floatHost,
	tabModel,
	onCloseBar,
	closeShortcutLabel,
	shortcutPlatform,
	onReturnFocus,
}: WorkspaceFilesSurfaceProps): React.ReactNode {
	const [windows, dispatch] = React.useReducer(reduceFloatWindows, new Map());
	// The files the float windows edit, one model each, kept while any tab shows them so edits
	// survive collapsing the window or switching Workspace.
	const [documents] = React.useState(() => new EditorDocuments());
	React.useEffect(() => () => documents.dispose(), [documents]);
	React.useEffect(() => {
		documents.retain(
			new Map(
				[...windows].map(([id, state]) => [
					id,
					new Set(state.tabs.flatMap((tab) => documentPathOf(tab.target) ?? [])),
				]),
			),
		);
	}, [documents, windows]);
	// The first edit keeps a preview tab, so opening another file does not replace it.
	React.useEffect(
		() =>
			documents.subscribe(() => {
				for (const [id, paths] of documents.dirtyByWorkspace()) {
					dispatch({ kind: 'keep-unsaved', paths, workspaceId: id });
				}
			}),
		[documents],
	);
	const [refreshCount, setRefreshCount] = React.useState(0);
	const [diffOptions, setDiffOptions] = React.useState(DEFAULT_DIFF_OPTIONS);
	const float = floatOf(windows, workspaceId);
	const activeTab = float.tabs.find((tab) => tab.id === float.activeId) ?? null;

	const activeTabId = React.useSyncExternalStore(
		tabModel?.subscribe ?? NO_TABS,
		tabModel === null ? NO_ACTIVE_TAB : () => tabModel.getSnapshot().activeTabId,
	);
	const shownTabId = React.useRef(activeTabId);
	React.useEffect(() => {
		if (shownTabId.current === activeTabId) return;
		shownTabId.current = activeTabId;
		if (workspaceId !== null) dispatch({ kind: 'collapse', workspaceId });
	}, [activeTabId, workspaceId]);

	const open = useMemoizedFn((target: FloatTarget, preview: boolean) => {
		if (workspaceId !== null) {
			const unsaved = documents.dirtyPaths(workspaceId);
			dispatch({ kind: 'open', preview, target, unsaved, workspaceId });
		}
	});
	const refresh = useMemoizedFn(() => setRefreshCount((count) => count + 1));

	// Each Workspace keeps its open folders and its dismissed note while the window lives.
	const [folders, setFolders] = React.useState<ReadonlyMap<string, ReadonlySet<string>>>(
		() => new Map(),
	);
	const [noteDismissed, setNoteDismissed] = React.useState<ReadonlySet<string>>(() => new Set());
	const expanded = (workspaceId === null ? undefined : folders.get(workspaceId)) ?? NO_FOLDERS;

	// What shows is followed: the open bar's folders, and the expanded float window's files. A
	// collapsed float window or a closed bar reads again when it shows, so neither is followed.
	const barOpen = barHost !== null;
	const scope = React.useMemo(() => {
		if (!barOpen && !float.open) return null;
		return {
			directories: barOpen ? ['', ...[...expanded].sort(byDepth)] : [],
			files: float.open
				? float.tabs.flatMap((tab) => (tab.target.kind === 'file' ? [tab.target.path] : []))
				: [],
		};
	}, [barOpen, expanded, float.open, float.tabs]);

	// A change reads again only what shows it: the bar's view, and the float window's tab when it
	// shows that file or a diff, which the repository's status covers.
	const [autoRefresh, setAutoRefresh] = React.useState<AutoRefresh>({
		count: 0,
		files: false,
		repository: false,
	});
	const showChange = useMemoizedFn((change: WorkspaceFilesChange) => {
		if (barOpen) setAutoRefresh((previous) => ({ count: previous.count + 1, ...change }));
		const target = float.open ? activeTab?.target : undefined;
		if (target !== undefined && (target.kind === 'file' ? change.files : change.repository)) {
			refresh();
		}
	});
	const watch = useWorkspaceFilesWatch(source, workspaceId, scope, showChange);
	useAgentSettled(agentSource, () => watch.notify(EVERYTHING));

	return (
		<>
			{barHost === null
				? null
				: createPortal(
						<RightBar
							activeTarget={float.open ? (activeTab?.target ?? null) : null}
							autoRefresh={autoRefresh}
							closeShortcutLabel={closeShortcutLabel}
							expanded={expanded}
							key={workspaceId ?? ''}
							noteDismissed={workspaceId !== null && noteDismissed.has(workspaceId)}
							onClose={onCloseBar}
							onDismissNote={() => {
								if (workspaceId !== null) {
									setNoteDismissed((previous) => new Set(previous).add(workspaceId));
								}
							}}
							onExpandedChange={(next) => {
								if (workspaceId !== null) {
									setFolders((previous) => new Map(previous).set(workspaceId, next));
								}
							}}
							onOpen={open}
							onRefresh={refresh}
							source={source}
							watchState={watch.state}
							workspaceId={workspaceId}
						/>,
						barHost,
					)}
			{floatHost === null || workspaceId === null
				? null
				: createPortal(
						<FloatWindow
							diffOptions={diffOptions}
							documents={documents}
							key={workspaceId}
							onActivate={(id) => dispatch({ id, kind: 'activate', workspaceId })}
							onClose={(ids) => dispatch({ ids, kind: 'close', workspaceId })}
							onCollapse={() => dispatch({ kind: 'collapse', workspaceId })}
							onDiffOptionsChange={setDiffOptions}
							onExpand={() => dispatch({ kind: 'expand', workspaceId })}
							onKeep={(id) => dispatch({ id, kind: 'keep', workspaceId })}
							onOpenFile={(path) => open({ kind: 'file', path }, false)}
							onReturnFocus={onReturnFocus}
							refreshCount={refreshCount}
							shortcutPlatform={shortcutPlatform}
							source={source}
							state={float}
							workspaceId={workspaceId}
						/>,
						floatHost,
					)}
		</>
	);
}
