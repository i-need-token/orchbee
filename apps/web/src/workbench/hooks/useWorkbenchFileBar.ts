import {
	createTypedActionRuntime,
	type ActionResult,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileBarMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ResizablePanelHandle } from '@orchbee/ui/components';
import * as React from 'react';

import {
	DEFAULT_FILE_BAR_LAYOUT,
	FILE_BAR_WIDTH,
	hideFileBarToggle,
	keepFileBarToggle,
	loadFileBar,
	readFileBarLayout,
	shownFileBarToggles,
	writeFileBarLayout,
	type FileBarLayout,
	type FileBarLayoutStorage,
	type FileBarModule,
	type FileBarPosition,
	type FileBarRowMenuEntry,
	type FileBarViewId,
} from '../../file-bar';
import type { FileSessionRegistry, FileWorkspaceSession } from '../../file-session';
import { readSetting, type UserSettingsControl } from '../../user-settings';
import { registerWorkbenchFileBarActions } from '../actions/registerWorkbenchFileBarActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../actions/workbench-action-ids';
import type { WorkbenchFocusRouting } from '../types/workbench-focus-routing';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import { buildFileBarRowMenu, showFileBarViewActionId } from '../utils/file-bar-row-menu';
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
const NO_SETTINGS_CHANGES = (): (() => void) => () => undefined;

interface ResizableLayoutChangedMeta {
	readonly isUserInteraction: boolean;
}

interface ResizablePanelSize {
	readonly inPixels: number;
}

/** The File Bar docked beside the terminal area, as the Workbench lays it out. */
interface WorkbenchFileBarColumn {
	readonly activeView: FileBarViewId;
	/** Attaches the column's landmark, which tells whether focus is inside the File Bar. */
	readonly attachElement: (element: HTMLElement | null) => (() => void) | undefined;
	/** The loaded column, or null while its chunk loads or after it failed to. */
	readonly content: FileBarModule['FileBarColumn'] | null;
	readonly defaultSize: string;
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
	/** The toggles row's menu as it opens. */
	readonly rowMenu: () => readonly FileBarRowMenuEntry[];
	/** The active Workspace's file session the File Bar holds; null until the hold has started. */
	readonly session: FileWorkspaceSession | null;
	/** The view toggles the row shows, in its order. */
	readonly toggles: readonly FileBarViewId[];
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
	/** Follows the panel group the File Bar shares with the terminal area. */
	readonly onLayoutChanged: (
		layout: Readonly<Record<string, number>>,
		meta: ResizableLayoutChangedMeta,
	) => void;
	/** The side of the terminal area the File Bar takes, as the user's settings name it. */
	readonly position: FileBarPosition;
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
	/** The Client's file sessions, which the Workbench shares; without them the File Bar opens none. */
	readonly sessions: FileSessionRegistry | null;
	/** The shortcut the toggles row's menu shows for an Action, as VS Code's menus show keybindings. */
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	/** Decides the File Bar's minimum: Windows and Linux draw caption buttons over its row. */
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	/** Adds the docked File Bar to the region cycle. */
	readonly supplyFocusRegion: WorkbenchFocusRouting['supplyRegion'];
	/** Where the File Bar keeps its layout on this device. */
	readonly storage: FileBarLayoutStorage | undefined;
	/**
	 * Called as a control in the File Bar's row acts, a toggle, the File Bar's button or an item of the
	 * row's menu, to take away what covers the File Bar's views: the file overlay's full tier.
	 */
	readonly uncoverViews: () => void;
	/**
	 * The user's settings, which name the File Bar's side and take the moves; without them it stays
	 * after the terminal area and offers no move.
	 */
	readonly userSettings: UserSettingsControl | undefined;
	/** The active Workspace, whose files the File Bar shows; null outside a Workspace. */
	readonly workspaceId: string | null;
}

function containsFocus(element: HTMLElement | null): boolean {
	return element !== null && element.contains(element.ownerDocument.activeElement);
}

function sameLayout(left: FileBarLayout, right: FileBarLayout): boolean {
	return (
		left.activeView === right.activeView &&
		left.hiddenToggles.length === right.hiddenToggles.length &&
		left.hiddenToggles.every((view) => right.hiddenToggles.includes(view)) &&
		left.visible === right.visible &&
		left.width === right.width
	);
}

/**
 * The File Bar's place in the Workbench: its column on the side of the terminal area the user's
 * settings name, its width, whether it shows, its active view and its button. It starts as this
 * device last left it, hidden at first, and the first show loads its views through the File Bar's
 * lazy entry. It holds the active Workspace's file session once it has shown that Workspace in this
 * run, hidden or not, so a session starts only for a Workspace whose files it showed, and the count
 * badge follows it from then on.
 */
export function useWorkbenchFileBar({
	actionRuntime,
	actionScope,
	focusTerminal,
	mobile,
	offered,
	reportActionOutcome,
	sessions,
	shortcutLabelForAction,
	shortcutPlatform,
	storage,
	supplyFocusRegion,
	uncoverViews,
	userSettings,
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
	// Whether the keys were inside the column when it last left the document, as a move takes it out.
	const focusLeftWithColumn = React.useRef(false);
	const attachElement = useMemoizedFn((element: HTMLElement | null) => {
		elementReference.current = element;
		if (element === null) return undefined;
		// A landmark detaches before it leaves the document, while the keys are still where they were.
		return () => {
			focusLeftWithColumn.current = containsFocus(element);
			elementReference.current = null;
		};
	});
	const [viewFocusRequest, setViewFocusRequest] = React.useState(0);
	const [load, setLoad] = React.useState<FileBarLoad>(IDLE_LOAD);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
		[actionRuntime],
	);
	const subscribeSettings = React.useMemo(
		() =>
			userSettings === undefined
				? NO_SETTINGS_CHANGES
				: (listener: () => void) => userSettings.subscribe(listener),
		[userSettings],
	);
	const readPosition = (): FileBarPosition =>
		userSettings === undefined
			? 'right'
			: readSetting(userSettings.getSnapshot().snapshot, 'appearance.file_bar.position');
	const position = React.useSyncExternalStore(subscribeSettings, readPosition, readPosition);
	// The File Bar moves once the settings file answers with its side, as every Client reads it there.
	const move = React.useMemo(
		() =>
			userSettings === undefined
				? undefined
				: async (target: FileBarPosition): Promise<ActionResult<void>> => {
						const result = await userSettings.set('appearance.file_bar.position', target);
						switch (result.kind) {
							case 'current':
							case 'edited':
								return { kind: 'completed' };
							case 'outcome-unknown':
								return { failure: { code: 'outcome-unknown' }, kind: 'failed' };
							case 'refused':
								return { failure: { code: 'settings-refused' }, kind: 'failed' };
							case 'unavailable':
								return { failure: { code: 'settings-unavailable' }, kind: 'failed' };
						}
					},
		[userSettings],
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
		uncoverViews();
		if (view === layoutReference.current.activeView) {
			setViewFocusRequest((request) => request + 1);
			return;
		}
		reportActionOutcome('button', () =>
			typedRuntime.execute(showFileBarViewActionId(view), undefined, {
				scope: actionScope,
				source: 'button',
			}),
		);
	});
	// VS Code's activity bar menu pins and unpins a container: a toggle kept again opens its view with
	// the keys. Hiding the active view's toggle opens Explorer and leaves the keys where they are, or,
	// when Explorer's own toggle goes, opens the next view with the keys, as VS Code reopens its
	// default container without focus and any other with it.
	const setToggleShown = useMemoizedFn((view: FileBarViewId, shown: boolean) => {
		const current = layoutReference.current;
		if (!shown) {
			const next = hideFileBarToggle(current, view);
			commit(next);
			if (
				next.activeView !== current.activeView &&
				next.activeView !== DEFAULT_FILE_BAR_LAYOUT.activeView
			) {
				setViewFocusRequest((request) => request + 1);
			}
			return;
		}
		commit(keepFileBarToggle(current, view));
		setViewFocusRequest((request) => request + 1);
	});
	const rowMenu = useMemoizedFn(() =>
		buildFileBarRowMenu({
			formatMessage: (descriptor) => intl.formatMessage(descriptor),
			layout: layoutReference.current,
			position,
			report: reportActionOutcome,
			scope: actionScope,
			setToggleShown,
			shortcutLabelForAction,
			typedRuntime,
		}).map((entry) =>
			entry.kind === 'separator'
				? entry
				: {
						...entry,
						run: () => {
							uncoverViews();
							entry.run();
						},
					},
		),
	);
	const toggles = React.useMemo(() => shownFileBarToggles(layout), [layout]);
	// The button also uncovers a hidden File Bar's place, so the File Bar it shows is not beneath the
	// full tier.
	const invokeToggle = useMemoizedFn(() => {
		uncoverViews();
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
		// Moving the divider away from the File Bar widens it, on either side.
		const outward = position === 'right' ? -1 : 1;
		switch (event.key) {
			case 'ArrowLeft':
				event.preventDefault();
				setWidth(liveWidth.current - outward * DIVIDER_KEY_STEP);
				return;
			case 'ArrowRight':
				event.preventDefault();
				setWidth(liveWidth.current + outward * DIVIDER_KEY_STEP);
				return;
			case 'End':
				setWidth(position === 'right' ? minimumWidth : FILE_BAR_WIDTH.maximum);
				return;
			case 'Home':
				setWidth(position === 'right' ? FILE_BAR_WIDTH.maximum : minimumWidth);
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
			// A drag of the File Bar's divider commits once, on release.
			if (meta.isUserInteraction) {
				if (widthShare !== undefined && widthShareChanged) {
					commit({ ...layoutReference.current, width: clampWidth(liveWidth.current) });
				}
				return;
			}
			if (!panelSetChanged) return;
			// When the File Bar shows, hides or moves, the panel group restores the layout it last saw
			// for that set of panels: shares of a width that may have changed since. The File Bar goes
			// back to its own width.
			panelReference.current?.resize(layoutReference.current.width);
		},
	);
	// The mobile layout has no room to dock it yet, so it offers no Action there.
	const reachable = offered && !mobile;
	React.useEffect(() => {
		if (!reachable) return undefined;
		const registration = registerWorkbenchFileBarActions({
			isVisible: layout.visible,
			move,
			position,
			scope: actionScope,
			showView,
			toggle,
			typedRuntime,
		});
		return () => registration.dispose();
	}, [actionScope, layout.visible, move, position, reachable, showView, toggle, typedRuntime]);
	const docked = reachable && layout.visible;
	// A column that stops docking, hidden or left for the mobile layout, drops its focus request, and
	// so does one that moves to the other side: it mounts afresh where it docks next and would
	// otherwise take focus then.
	const [dockedForFocus, setDockedForFocus] = React.useState(docked);
	const [positionForFocus, setPositionForFocus] = React.useState(position);
	if (dockedForFocus !== docked || positionForFocus !== position) {
		if (!docked || positionForFocus !== position) setViewFocusRequest(0);
		setDockedForFocus(docked);
		setPositionForFocus(position);
	}
	// A move mounts the column afresh on its other side, so keys it held go to its active view there.
	const placedPosition = React.useRef(position);
	React.useLayoutEffect(() => {
		if (placedPosition.current === position) return;
		placedPosition.current = position;
		const focusLeft = focusLeftWithColumn.current;
		focusLeftWithColumn.current = false;
		if (docked && focusLeft) setViewFocusRequest((request) => request + 1);
	}, [docked, position]);
	// The docked File Bar takes its place in the region cycle, where its active view takes the keys.
	React.useEffect(() => {
		if (!docked) return undefined;
		return supplyFocusRegion('file-bar', () => setViewFocusRequest((request) => request + 1));
	}, [docked, supplyFocusRegion]);
	// The Workspaces the File Bar has shown in this run, whose sessions it keeps for the count badge.
	const [shownWorkspaces, setShownWorkspaces] = React.useState<ReadonlySet<string>>(NO_WORKSPACES);
	if (docked && workspaceId !== null && !shownWorkspaces.has(workspaceId)) {
		setShownWorkspaces(new Set([...shownWorkspaces, workspaceId]));
	}
	const holding = reachable && workspaceId !== null && (docked || shownWorkspaces.has(workspaceId));
	const [session, setSession] = React.useState<FileWorkspaceSession | null>(null);
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
	if (!docked) return { column: null, onLayoutChanged, position, toggle: toggleView };
	return {
		column: {
			activeView: layout.activeView,
			attachElement,
			content: load.kind === 'ready' ? load.module.FileBarColumn : null,
			defaultSize: `${layout.width}px`,
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
			rowMenu,
			session,
			toggles,
			viewFocusRequest,
		},
		onLayoutChanged,
		position,
		toggle: toggleView,
	};
}
