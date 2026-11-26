import { useMemoizedFn } from '@orchbee/foundation/react';
import type { TabModel } from '@orchbee/tab-strip/model';
import * as React from 'react';
import { createPortal } from 'react-dom';

import type { WorkbenchAgentSource, WorkbenchWorkspaceFilesSource } from '../workbench';
import type { DiffOptions } from './components/float-window/components/editor-pane/EditorPane';
import { FloatWindow } from './components/float-window/FloatWindow';
import { floatOf, reduceFloatWindows } from './float-reducer';
import type { FloatTarget } from './float-tabs';
import { RightBar } from './components/right-bar/RightBar';
import { useAgentSettled } from './hooks/useAgentSettled';

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
	onReturnFocus,
}: WorkspaceFilesSurfaceProps): React.ReactNode {
	const [windows, dispatch] = React.useReducer(reduceFloatWindows, new Map());
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
		if (workspaceId !== null) dispatch({ kind: 'open', preview, target, workspaceId });
	});
	const refresh = useMemoizedFn(() => setRefreshCount((count) => count + 1));
	// Reads what shows again when the files likely changed: the open bar reads its view and the
	// float window with it; with the bar closed, only an expanded float window reads. Nothing reads
	// while neither shows, or while the window is hidden, which reads again when it comes back.
	const [barRefreshCount, setBarRefreshCount] = React.useState(0);
	const invalidate = useMemoizedFn(() => {
		if (document.hidden) return;
		if (barHost !== null) setBarRefreshCount((count) => count + 1);
		else if (float.open) refresh();
	});
	useAgentSettled(agentSource, invalidate);

	return (
		<>
			{barHost === null
				? null
				: createPortal(
						<RightBar
							activeTarget={float.open ? (activeTab?.target ?? null) : null}
							autoRefreshCount={barRefreshCount}
							closeShortcutLabel={closeShortcutLabel}
							key={workspaceId ?? ''}
							onClose={onCloseBar}
							onOpen={open}
							onRefresh={refresh}
							source={source}
							workspaceId={workspaceId}
						/>,
						barHost,
					)}
			{floatHost === null || workspaceId === null
				? null
				: createPortal(
						<FloatWindow
							diffOptions={diffOptions}
							key={workspaceId}
							onActivate={(id) => dispatch({ id, kind: 'activate', workspaceId })}
							onClose={(ids) => dispatch({ ids, kind: 'close', workspaceId })}
							onCollapse={() => dispatch({ kind: 'collapse', workspaceId })}
							onDiffOptionsChange={setDiffOptions}
							onExpand={() => dispatch({ kind: 'expand', workspaceId })}
							onKeep={(id) => dispatch({ id, kind: 'keep', workspaceId })}
							onReturnFocus={onReturnFocus}
							refreshCount={refreshCount}
							source={source}
							state={float}
							workspaceId={workspaceId}
						/>,
						floatHost,
					)}
		</>
	);
}
