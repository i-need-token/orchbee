import { createTypedActionRuntime, type ActionRuntime, type ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileBarMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ResizablePanelHandle } from '@orchbee/ui/components';
import * as React from 'react';

import {
	DEFAULT_FILE_BAR_LAYOUT,
	FILE_BAR_WIDTH,
	loadFileBar,
	readFileBarLayout,
	writeFileBarLayout,
	type FileBarLayout,
	type FileBarLayoutStorage,
	type FileBarModule,
	type FileBarViewId,
} from '../../file-bar';
import {
	FileSessionRegistry,
	type FileSessionPort,
	type FileWorkspaceSession,
} from '../../file-session';
import { registerWorkbenchFileBarActions } from '../actions/registerWorkbenchFileBarActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../actions/workbench-action-ids';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import type { WorkbenchAuthorityNoticeView } from './useWorkbenchAuthorityNotice';

/** The File Bar's panel in the Workbench's resizable group. */
const FILE_BAR_PANEL_ID = 'file-bar';
/** What one ← or → on the focused divider moves it, as on the Sidebar's. */
const DIVIDER_KEY_STEP = 10;

type FileBarLoad =
	| { readonly kind: 'failed' }
	| { readonly kind: 'idle' }
	| { readonly kind: 'ready'; readonly module: FileBarModule };

const IDLE_LOAD: FileBarLoad = { kind: 'idle' };
const NO_WORKSPACES: ReadonlySet<string> = new Set();

interface ResizableLayoutChangedMeta {
	readonly isUserInteraction: boolean;
}

interface ResizablePanelSize {
	readonly inPixels: number;
}

/** The File Bar docked beside the terminal area, as the Workbench lays it out. */
interface WorkbenchFileBarColumn {
	readonly activeView: FileBarViewId;
	/** The loaded column, or null while its chunk loads or after it failed to. */
	readonly content: FileBarModule['FileBarColumn'] | null;
	readonly defaultSize: string;
	/** The column's landmark, which tells whether focus is inside the File Bar. */
	readonly elementReference: React.RefObject<HTMLElement | null>;
	/** What the column says instead of its views when they could not load. */
	readonly failure: string | null;
	readonly id: string;
	readonly label: string;
	readonly maxSize: string;
	readonly minSize: string;
	/** Double-clicking the divider returns the File Bar to its default width. */
	readonly onDividerDoubleClick: () => void;
	/** Modifier-free ← and → on the focused divider change the width in 10px steps. */
	readonly onDividerKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
	readonly onResize: (size: ResizablePanelSize) => void;
	/** Selecting a view's toggle in the column. */
	readonly onSelectView: (view: FileBarViewId) => void;
	readonly panelReference: React.RefObject<ResizablePanelHandle | null>;
	/** The active Workspace's file session the File Bar holds; null until the hold has started. */
	readonly session: FileWorkspaceSession | null;
	/** Each increase moves focus into the active view. */
	readonly viewFocusRequest: number;
}

/** Source Control's count badge on the File Bar's button, from the File Bar's chunk. */
interface WorkbenchFileBarToggleBadge {
	readonly Badge: FileBarModule['FileBarToggleBadge'];
	readonly session: FileWorkspaceSession;
}

/** The button that shows and hides the File Bar. */
interface WorkbenchFileBarToggle {
	/**
	 * Present while the File Bar is hidden after it has shown the active Workspace in this run, so the
	 * button carries the count its Source Control toggle would.
	 */
	readonly badge: WorkbenchFileBarToggleBadge | null;
	readonly label: string;
	readonly onToggle: () => void;
	readonly shown: boolean;
}

export interface WorkbenchFileBarView {
	/** Present while the window offers the File Bar and it shows docked. */
	readonly column: WorkbenchFileBarColumn | null;
	/** Follows the Workbench's panel group, which the File Bar shares with the Sidebar. */
	readonly onLayoutChanged: (
		layout: Readonly<Record<string, number>>,
		meta: ResizableLayoutChangedMeta,
	) => void;
	/** Present while the window offers the File Bar and has room to dock it. */
	readonly toggle: WorkbenchFileBarToggle | null;
}

interface UseWorkbenchFileBarOptions {
	readonly actionRuntime: ActionRuntime;
	/** The Workbench's own scope, where the File Bar's Actions resolve. */
	readonly actionScope: ActionScope;
	/** Gives keyboard focus back to the current terminal. */
	readonly focusTerminal: () => void;
	/** The mobile layout, which has no room to dock the File Bar. */
	readonly mobile: boolean;
	/** Whether this window offers the File Bar; without the offer nothing registers, reads or loads. */
	readonly offered: boolean;
	readonly reportActionOutcome: WorkbenchAuthorityNoticeView['reportActionOutcome'];
	/** Lays the docked Sidebar back at its own width. */
	readonly restoreSidebarWidth: () => void;
	/** Where file sessions come from; without it the File Bar opens none. */
	readonly sessionPort: FileSessionPort | undefined;
	/** Decides the File Bar's minimum: Windows and Linux draw caption buttons over its row. */
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	/** Where the File Bar keeps its layout on this device. */
	readonly storage: FileBarLayoutStorage | undefined;
	/** The active Workspace, whose files the File Bar shows; null outside a Workspace. */
	readonly workspaceId: string | null;
}

function containsFocus(element: HTMLElement | null): boolean {
	return element !== null && element.contains(element.ownerDocument.activeElement);
}

function sameLayout(left: FileBarLayout, right: FileBarLayout): boolean {
	return (
		left.activeView === right.activeView &&
		left.visible === right.visible &&
		left.width === right.width
	);
}

function showViewActionId(view: FileBarViewId) {
	return view === 'explorer'
		? WORKBENCH_ACTION_IDS.showExplorer
		: WORKBENCH_ACTION_IDS.showSourceControl;
}

/**
 * The File Bar's place in the Workbench: its column after the terminal area, its width, whether it
 * shows, its active view and its button. It starts as this device last left it, hidden at first, and
 * the first show loads its views through the File Bar's lazy entry. It holds the active Workspace's
 * file session once it has shown that Workspace in this run, hidden or not, so a session starts only
 * for a Workspace whose files it showed, and the count badge follows it from then on.
 */
export function useWorkbenchFileBar({
	actionRuntime,
	actionScope,
	focusTerminal,
	mobile,
	offered,
	reportActionOutcome,
	restoreSidebarWidth,
	sessionPort,
	shortcutPlatform,
	storage,
	workspaceId,
}: UseWorkbenchFileBarOptions): WorkbenchFileBarView {
	const intl = useOrchbeeIntl();
	const minimumWidth =
		shortcutPlatform === 'apple'
			? FILE_BAR_WIDTH.minimum
			: FILE_BAR_WIDTH.minimumBesideCaptionButtons;
	const clampWidth = (width: number): number =>
		Math.min(FILE_BAR_WIDTH.maximum, Math.max(minimumWidth, Math.round(width)));
	const [layout, setLayout] = React.useState<FileBarLayout>(() => {
		if (!offered) return DEFAULT_FILE_BAR_LAYOUT;
		const stored = readFileBarLayout(storage);
		return { ...stored, width: clampWidth(stored.width) };
	});
	// The committed layout as handlers last left it, ahead of the render that shows it.
	const layoutReference = React.useRef(layout);
	// The width the panel reports while it resizes, committed when a resize completes.
	const liveWidth = React.useRef(layout.width);
	const lastWidthShare = React.useRef<number | undefined>(undefined);
	const panelSet = React.useRef<string | null>(null);
	const panelReference = React.useRef<ResizablePanelHandle>(null);
	const elementReference = React.useRef<HTMLElement>(null);
	const [viewFocusRequest, setViewFocusRequest] = React.useState(0);
	const [load, setLoad] = React.useState<FileBarLoad>(IDLE_LOAD);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
		[actionRuntime],
	);
	const commit = useMemoizedFn((next: FileBarLayout) => {
		if (sameLayout(layoutReference.current, next)) return;
		layoutReference.current = next;
		setLayout(next);
		writeFileBarLayout(storage, next);
	});
	const prepareShow = useMemoizedFn(() => {
		// A failed load tries again the next time the File Bar shows.
		setLoad((current) => (current.kind === 'failed' ? IDLE_LOAD : current));
		liveWidth.current = layoutReference.current.width;
	});
	const toggle = useMemoizedFn(() => {
		const current = layoutReference.current;
		if (!current.visible) {
			prepareShow();
			commit({ ...current, visible: true });
			return;
		}
		// Focus inside the File Bar would leave with it, so it returns to the terminal first.
		if (containsFocus(elementReference.current)) focusTerminal();
		commit({ ...current, visible: false });
	});
	// VS Code's Show Explorer and Show Source Control: they show the view and focus it, and from the
	// focused view they return focus to the terminal, as VS Code's return to the editor.
	const showView = useMemoizedFn((view: FileBarViewId) => {
		const current = layoutReference.current;
		if (current.visible && current.activeView === view && containsFocus(elementReference.current)) {
			focusTerminal();
			return;
		}
		if (!current.visible) prepareShow();
		commit({ ...current, activeView: view, visible: true });
		setViewFocusRequest((request) => request + 1);
	});
	// The toggles row selects as VS Code's activity bar does at the top of its Side Bar: the active
	// view's toggle focuses the view, another one shows its view.
	const selectView = useMemoizedFn((view: FileBarViewId) => {
		if (view === layoutReference.current.activeView) {
			setViewFocusRequest((request) => request + 1);
			return;
		}
		reportActionOutcome('button', () =>
			typedRuntime.execute(showViewActionId(view), undefined, {
				scope: actionScope,
				source: 'button',
			}),
		);
	});
	const invokeToggle = useMemoizedFn(() => {
		reportActionOutcome('button', () =>
			typedRuntime.execute(WORKBENCH_ACTION_IDS.toggleFileBar, undefined, {
				scope: actionScope,
				source: 'button',
			}),
		);
	});
	const setWidth = useMemoizedFn((width: number) => {
		const panel = panelReference.current;
		if (panel === null) return;
		const next = clampWidth(width);
		panel.resize(next);
		liveWidth.current = next;
		commit({ ...layoutReference.current, width: next });
	});
	const onDividerKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLDivElement>) => {
		if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
		// The panel group has already moved the divider for these keys; the File Bar takes its own
		// steps, and Home and End keep the group's widest and narrowest File Bar as they commit it.
		switch (event.key) {
			case 'ArrowLeft':
				event.preventDefault();
				setWidth(liveWidth.current + DIVIDER_KEY_STEP);
				return;
			case 'ArrowRight':
				event.preventDefault();
				setWidth(liveWidth.current - DIVIDER_KEY_STEP);
				return;
			case 'End':
				setWidth(minimumWidth);
				return;
			case 'Home':
				setWidth(FILE_BAR_WIDTH.maximum);
				return;
		}
	});
	const onDividerDoubleClick = useMemoizedFn(() => setWidth(FILE_BAR_WIDTH.default));
	const onResize = useMemoizedFn((size: ResizablePanelSize) => {
		liveWidth.current = size.inPixels;
	});
	const onLayoutChanged = useMemoizedFn(
		(panels: Readonly<Record<string, number>>, meta: ResizableLayoutChangedMeta) => {
			if (!offered) return;
			const nextPanelSet = Object.keys(panels).join(',');
			const panelSetChanged = nextPanelSet !== panelSet.current;
			panelSet.current = nextPanelSet;
			const widthShare = panels[FILE_BAR_PANEL_ID];
			const widthShareChanged = widthShare !== lastWidthShare.current;
			lastWidthShare.current = widthShare;
			if (meta.isUserInteraction) {
				// A drag of the File Bar's divider commits once, on release; a drag of the Sidebar's
				// leaves the File Bar's width as it was.
				if (widthShare !== undefined && widthShareChanged) {
					commit({ ...layoutReference.current, width: clampWidth(liveWidth.current) });
				}
				return;
			}
			if (!panelSetChanged) return;
			// When a column shows or hides, the panel group restores the layout it last saw for that
			// set of panels: shares of a window width that may have changed since, from before a later
			// resize of either column. Both columns go back to their own widths.
			panelReference.current?.resize(layoutReference.current.width);
			restoreSidebarWidth();
		},
	);
	// The mobile layout has no room to dock it yet, so it offers no Action there.
	const reachable = offered && !mobile;
	React.useEffect(() => {
		if (!reachable) return undefined;
		const registration = registerWorkbenchFileBarActions({
			isVisible: layout.visible,
			scope: actionScope,
			showView,
			toggle,
			typedRuntime,
		});
		return () => registration.dispose();
	}, [actionScope, layout.visible, reachable, showView, toggle, typedRuntime]);
	const docked = reachable && layout.visible;
	// A column that stops docking, hidden or left for the mobile layout, drops its focus request: the
	// column mounts afresh when it docks again and would otherwise take focus then.
	const [dockedForFocus, setDockedForFocus] = React.useState(docked);
	if (dockedForFocus !== docked) {
		setDockedForFocus(docked);
		if (!docked) setViewFocusRequest(0);
	}
	// The Workspaces the File Bar has shown in this run, whose sessions it keeps for the count badge.
	const [shownWorkspaces, setShownWorkspaces] = React.useState<ReadonlySet<string>>(NO_WORKSPACES);
	if (docked && workspaceId !== null && !shownWorkspaces.has(workspaceId)) {
		setShownWorkspaces(new Set([...shownWorkspaces, workspaceId]));
	}
	const holding = reachable && workspaceId !== null && (docked || shownWorkspaces.has(workspaceId));
	const [session, setSession] = React.useState<FileWorkspaceSession | null>(null);
	const sessions = React.useMemo(
		() => (sessionPort === undefined ? null : new FileSessionRegistry(sessionPort)),
		[sessionPort],
	);
	React.useEffect(() => {
		if (sessions === null || !holding || workspaceId === null) return undefined;
		const hold = sessions.hold(workspaceId);
		setSession(hold.session);
		return () => {
			setSession(null);
			hold.release();
		};
	}, [holding, sessions, workspaceId]);
	React.useEffect(() => {
		if (!docked || load.kind !== 'idle') return undefined;
		let current = true;
		void loadFileBar().then(
			(module) => {
				if (current) setLoad({ kind: 'ready', module });
			},
			() => {
				if (current) setLoad({ kind: 'failed' });
			},
		);
		return () => {
			current = false;
		};
	}, [docked, load.kind]);
	const toggleView = reachable
		? {
				badge:
					layout.visible || session === null || load.kind !== 'ready'
						? null
						: { Badge: load.module.FileBarToggleBadge, session },
				label: intl.formatMessage(
					layout.visible ? fileBarMessages.hideFileBar : fileBarMessages.showFileBar,
				),
				onToggle: invokeToggle,
				shown: layout.visible,
			}
		: null;
	if (!docked) return { column: null, onLayoutChanged, toggle: toggleView };
	return {
		column: {
			activeView: layout.activeView,
			content: load.kind === 'ready' ? load.module.FileBarColumn : null,
			defaultSize: `${layout.width}px`,
			elementReference,
			failure: load.kind === 'failed' ? intl.formatMessage(fileBarMessages.loadFailed) : null,
			id: FILE_BAR_PANEL_ID,
			label: intl.formatMessage(fileBarMessages.label),
			maxSize: `${FILE_BAR_WIDTH.maximum}px`,
			minSize: `${minimumWidth}px`,
			onDividerDoubleClick,
			onDividerKeyDown,
			onResize,
			onSelectView: selectView,
			panelReference,
			session,
			viewFocusRequest,
		},
		onLayoutChanged,
		toggle: toggleView,
	};
}
