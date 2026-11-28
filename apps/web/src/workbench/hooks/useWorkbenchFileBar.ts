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
import { isUnclaimedEscape } from '../utils/isUnclaimedEscape';
import { resolveWorkbenchFocusRegionFromTarget } from '../utils/workbench-focus-dom';
import type { WorkbenchAuthorityNoticeView } from './useWorkbenchAuthorityNotice';
import {
	useWorkbenchFileBarMoveDrag,
	type WorkbenchFileBarMoveCatcher,
} from './useWorkbenchFileBarMoveDrag';

/** The File Bar's panel in the Workbench's resizable group. */
const FILE_BAR_PANEL_ID = 'file-bar';
/** What one ← or → on the focused divider moves it, as on the Sidebar's. */
const DIVIDER_KEY_STEP = 10;
/** The divider between the docked File Bar and the terminal area. */
const DIVIDER_WIDTH = 1;
/**
 * What the terminal area keeps beside the docked File Bar: about 60 columns, which the Sidebar steps
 * aside to keep and the TUI hides its sidebar to keep.
 */
const TERMINAL_AREA_MINIMUM_WIDTH = 481;

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

/** Where the File Bar shows: docked on a side of the terminal area, over it, or nowhere. */
type FileBarPlacement = FileBarPosition | 'hidden' | 'overlay';

/**
 * Where the terminal area lies in the Workbench root while the File Bar opens over it: where it
 * starts, which changes only as the Sidebar docks or stops docking, and whether the File Bar's width
 * reaches across it. Either changes only at such a step, so a window resize between them renders
 * nothing.
 */
interface FileBarOverlayPlace {
	readonly across: boolean;
	readonly start: number;
}

/** What the File Bar shows wherever it shows: its views, their toggles and the row's menu. */
interface WorkbenchFileBarContent {
	readonly activeView: FileBarViewId;
	/** Attaches the File Bar's landmark, which tells whether focus is inside the File Bar. */
	readonly attachElement: (element: HTMLElement | null) => (() => void) | undefined;
	/** The loaded column, or null while its chunk loads or after it failed to. */
	readonly content: FileBarModule['FileBarColumn'] | null;
	/** What the column says instead of its views when they could not load. */
	readonly failure: string | null;
	readonly label: string;
	/** Selecting a view's toggle in the column. */
	readonly onSelectView: (view: FileBarViewId) => void;
	/** The toggles row's menu as it opens. */
	readonly rowMenu: () => readonly FileBarRowMenuEntry[];
	/** The active Workspace's file session the File Bar holds; null until the hold has started. */
	readonly session: FileWorkspaceSession | null;
	/** The view toggles the row shows, in its order. */
	readonly toggles: readonly FileBarViewId[];
	/** Each increase moves focus into the active view. */
	readonly viewFocusRequest: number;
}

/** The File Bar docked beside the terminal area, as the Workbench lays it out. */
interface WorkbenchFileBarColumn extends WorkbenchFileBarContent {
	readonly defaultSize: string;
	readonly id: string;
	readonly maxSize: string;
	readonly minSize: string;
	/** Double-clicking the divider returns the File Bar to its default width. */
	readonly onDividerDoubleClick: () => void;
	/** Modifier-free ← and → on the focused divider change the width in 10px steps. */
	readonly onDividerKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
	readonly onResize: (size: ResizablePanelSize) => void;
	/**
	 * A press on the toggles row, which a drag turns into moving the File Bar to the other side of the
	 * terminal area; null where the window has no settings to keep its side in.
	 */
	readonly onRowPointerDown: ((event: React.PointerEvent<HTMLElement>) => void) | null;
	readonly panelReference: React.RefObject<ResizablePanelHandle | null>;
	/**
	 * The terminal area's minimum beside the File Bar, which the File Bar gives way to: as the room
	 * narrows it narrows first, down to its own minimum, and no drag widens it past the room.
	 */
	readonly terminalAreaMinSize: string;
}

/**
 * The File Bar opened over the terminal area from its side without room to dock, or filling the
 * window in the mobile layout.
 */
interface WorkbenchFileBarOverlay extends WorkbenchFileBarContent {
	/** Whether it reaches the window's end, where its row keeps the caption buttons' place. */
	readonly atWindowEnd: boolean;
	/** Whether it starts at the window's start, where its row keeps the Sidebar button's place. */
	readonly atWindowStart: boolean;
	/** Whether it fills the window, as the sidebar list does in the mobile layout. */
	readonly fill: boolean;
	/**
	 * Where the terminal area starts in the Workbench root: it opens from there on the left, and on
	 * either side reaches no further.
	 */
	readonly inlineStart: number;
	/** Esc that nothing inside has taken closes it and gives the terminal the keys. */
	readonly onKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
	/** A press outside it closes it and gives the terminal the keys. */
	readonly onScrimPointerDown: (event: React.PointerEvent<HTMLDivElement>) => void;
	/** Its own width, which it keeps as far as the terminal area reaches. */
	readonly width: number;
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
	/** The panel group the File Bar shares with the terminal area, whose width decides the dock. */
	readonly areaReference: React.RefObject<HTMLDivElement | null>;
	/** Present while the window offers the File Bar and it shows docked. */
	readonly column: WorkbenchFileBarColumn | null;
	/**
	 * Called as a file opens: the File Bar open over the terminal area closes, and answers whether it
	 * held the keys, which the file's tab then takes.
	 */
	readonly fileOpened: () => boolean;
	/** Present while a drag of the docked File Bar's toggles row runs. */
	readonly moveCatcher: WorkbenchFileBarMoveCatcher | null;
	/** Follows the panel group the File Bar shares with the terminal area. */
	readonly onLayoutChanged: (
		layout: Readonly<Record<string, number>>,
		meta: ResizableLayoutChangedMeta,
	) => void;
	/**
	 * Present while the File Bar shows over the terminal area without room to dock, or fills the
	 * window in the mobile layout.
	 */
	readonly overlay: WorkbenchFileBarOverlay | null;
	/** The side of the terminal area the File Bar takes, as the user's settings name it. */
	readonly position: FileBarPosition;
	/** Present while the window offers the File Bar. */
	readonly toggle: WorkbenchFileBarToggle | null;
}

interface UseWorkbenchFileBarOptions {
	readonly actionRuntime: ActionRuntime;
	/** The Workbench's own scope, where the File Bar's Actions resolve. */
	readonly actionScope: ActionScope;
	/**
	 * Closes the Sidebar and the Tab list over the content, which give way as the File Bar opens over
	 * the terminal area.
	 */
	readonly closeSurfacesOverContent: () => void;
	/** Gives keyboard focus back to the current terminal. */
	readonly focusTerminal: () => void;
	/** The mobile layout, where the File Bar never docks and fills the window as it opens. */
	readonly mobile: boolean;
	/** Whether this window offers the File Bar; without the offer nothing registers, reads or loads. */
	readonly offered: boolean;
	readonly reportActionOutcome: WorkbenchAuthorityNoticeView['reportActionOutcome'];
	/** The Workbench root, in which the File Bar opens over the terminal area. */
	readonly rootReference: React.RefObject<HTMLElement | null>;
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
	 * Whether the Sidebar or the Tab list lies over the content; the File Bar open over the terminal
	 * area gives way as one opens.
	 */
	readonly surfacesOverContent: boolean;
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
 *
 * It docks while the terminal area keeps 481px beside it at its narrowest, narrowing first as the
 * room does. With less room it steps aside, as the Sidebar does in a narrow window: its button opens
 * it at its width over the terminal area from its side, where it takes the keys until Esc, a press
 * outside or a file opening closes it. Only a docked choice persists, so room regained docks it again
 * as it was left. In the mobile layout it opens filling the window, as the sidebar list does.
 */
export function useWorkbenchFileBar({
	actionRuntime,
	actionScope,
	closeSurfacesOverContent,
	focusTerminal,
	mobile,
	offered,
	reportActionOutcome,
	rootReference,
	sessions,
	shortcutLabelForAction,
	shortcutPlatform,
	storage,
	supplyFocusRegion,
	surfacesOverContent,
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
	const panelReference = React.useRef<ResizablePanelHandle>(null);
	const areaReference = React.useRef<HTMLDivElement>(null);
	const elementReference = React.useRef<HTMLElement>(null);
	// Whether the keys were inside the File Bar when it last left the document, as it leaves its place.
	const focusLeftWithColumn = React.useRef(false);
	// Set as a file opens with the keys in the File Bar over the terminal area: the file's tab takes them.
	const keysWentToFile = React.useRef(false);
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
	// Whether the panel group the File Bar shares with the terminal area leaves that area its minimum
	// beside the File Bar at its narrowest; unmeasured, it does.
	const [roomToDock, setRoomToDock] = React.useState(true);
	const [overlayOpen, setOverlayOpen] = React.useState(false);
	// The mobile layout never docks it, whatever room its panel group reports.
	const stepsAside = mobile || !roomToDock;
	// Gaining or losing the room to dock closes the File Bar over the terminal area or the window, so
	// it docks again as it was left, as the Sidebar does across its docking width, while crossing the
	// mobile width alone keeps it open in the other form, as it keeps the Sidebar; a surface opening
	// over the content in its place closes it too.
	const [stepsAsideSeen, setStepsAsideSeen] = React.useState(stepsAside);
	if (stepsAsideSeen !== stepsAside) {
		setStepsAsideSeen(stepsAside);
		setOverlayOpen(false);
	}
	const [surfacesSeen, setSurfacesSeen] = React.useState(surfacesOverContent);
	if (surfacesSeen !== surfacesOverContent) {
		setSurfacesSeen(surfacesOverContent);
		if (surfacesOverContent) setOverlayOpen(false);
	}
	const dockable = offered && !stepsAside;
	const docked = dockable && layout.visible;
	const overlayShown = offered && stepsAside && overlayOpen;
	const shown = docked || overlayShown;
	// Over the terminal area the File Bar takes the keys as it opens, as the Sidebar over the content
	// does, and the Sidebar and the Tab list give way to it.
	const openOverlay = useMemoizedFn(() => {
		closeSurfacesOverContent();
		prepareShow();
		setOverlayOpen(true);
		setViewFocusRequest((request) => request + 1);
	});
	// The terminal it covers takes the keys, so the File Bar over the terminal area closes.
	const returnFromOverlay = useMemoizedFn(() => {
		focusTerminal();
		setOverlayOpen(false);
	});
	const onOverlayKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLDivElement>) => {
		if (!isUnclaimedEscape(event)) return;
		event.preventDefault();
		returnFromOverlay();
	});
	const onOverlayScrimPointerDown = useMemoizedFn((event: React.PointerEvent<HTMLDivElement>) => {
		// The scrim is gone before the press's mousedown, which would land on the Workbench root and
		// blur the terminal again, so the press is cancelled, as on the Sidebar's scrim.
		event.preventDefault();
		returnFromOverlay();
	});
	const fileOpened = useMemoizedFn((): boolean => {
		if (!overlayShown) return false;
		const keys = containsFocus(elementReference.current);
		keysWentToFile.current = keys;
		setOverlayOpen(false);
		return keys;
	});
	const toggle = useMemoizedFn(() => {
		const current = layoutReference.current;
		// Without room to dock, the button opens and closes the File Bar over the terminal area and
		// leaves the docked choice as it was.
		if (!dockable) {
			if (!overlayShown) {
				openOverlay();
				return;
			}
			if (containsFocus(elementReference.current)) focusTerminal();
			setOverlayOpen(false);
			return;
		}
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
		if (shown && current.activeView === view && containsFocus(elementReference.current)) {
			focusTerminal();
			setOverlayOpen(false);
			return;
		}
		if (!dockable) {
			commit({ ...current, activeView: view });
			if (overlayShown) setViewFocusRequest((request) => request + 1);
			else openOverlay();
			return;
		}
		if (!current.visible) prepareShow();
		commit({ ...current, activeView: view, visible: true });
		setViewFocusRequest((request) => request + 1);
	});
	// Over the terminal area every tier of the file overlay lies over the File Bar's views, which
	// shrink it as they take the keys, so only a docked File Bar's row controls take the full tier off.
	const uncover = (): void => {
		if (dockable) uncoverViews();
	};
	// The toggles row selects as VS Code's activity bar does at the top of its Side Bar: the active
	// view's toggle focuses the view, another one shows its view.
	const selectView = useMemoizedFn((view: FileBarViewId) => {
		uncover();
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
	const setToggleShown = useMemoizedFn((view: FileBarViewId, kept: boolean) => {
		const current = layoutReference.current;
		if (!kept) {
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
							uncover();
							entry.run();
						},
					},
		),
	);
	const toggles = React.useMemo(() => shownFileBarToggles(layout), [layout]);
	// The button also uncovers a hidden File Bar's place, so the File Bar it shows is not beneath the
	// full tier.
	const invokeToggle = useMemoizedFn(() => {
		uncover();
		reportActionOutcome('button', () =>
			typedRuntime.execute(WORKBENCH_ACTION_IDS.toggleFileBar, undefined, {
				scope: actionScope,
				source: 'button',
			}),
		);
	});
	// The widest the docked File Bar can be beside the terminal area's minimum; unmeasured, no limit.
	const roomWidth = (): number => {
		const width = areaReference.current?.getBoundingClientRect().width ?? 0;
		return width === 0
			? Number.POSITIVE_INFINITY
			: Math.floor(width - DIVIDER_WIDTH - TERMINAL_AREA_MINIMUM_WIDTH);
	};
	const setWidth = useMemoizedFn((width: number) => {
		const panel = panelReference.current;
		if (panel === null) return;
		// The width kept is the width shown, which leaves the terminal area its minimum.
		const next = Math.max(minimumWidth, Math.min(clampWidth(width), roomWidth()));
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
	// The docked File Bar goes back toward its own width, as far as the terminal area's minimum lets
	// it; a panel group not laid out yet has no width to share.
	const fitColumn = useMemoizedFn(() => {
		if ((areaReference.current?.getBoundingClientRect().width ?? 0) === 0) return;
		panelReference.current?.resize(layoutReference.current.width);
	});
	const onLayoutChanged = useMemoizedFn(
		(panels: Readonly<Record<string, number>>, meta: ResizableLayoutChangedMeta) => {
			if (!offered) return;
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
			// As the File Bar shows or moves, the panel group restores the layout it last saw for that
			// set of panels, in shares of a width that may have changed since.
			if (widthShare !== undefined) fitColumn();
		},
	);
	React.useEffect(() => {
		if (!offered) return undefined;
		const registration = registerWorkbenchFileBarActions({
			isVisible: shown,
			move,
			position,
			scope: actionScope,
			showView,
			toggle,
			typedRuntime,
		});
		return () => registration.dispose();
	}, [actionScope, move, offered, position, shown, showView, toggle, typedRuntime]);
	// The room the File Bar shares with the terminal area decides whether it docks. As the room
	// narrows, the terminal area's minimum takes from the docked File Bar, whose pixel width the panel
	// group then keeps as the room widens again, so the File Bar widens back itself, a frame later,
	// once the group has followed the room, which no group reports while a divider drags.
	React.useLayoutEffect(() => {
		const area = areaReference.current;
		if (!offered || area === null) return undefined;
		let frame = 0;
		const measure = (): void => {
			const { width } = area.getBoundingClientRect();
			setRoomToDock(
				width === 0 || width - DIVIDER_WIDTH - TERMINAL_AREA_MINIMUM_WIDTH >= minimumWidth,
			);
			cancelAnimationFrame(frame);
			frame = requestAnimationFrame(fitColumn);
		};
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(area);
		return () => {
			cancelAnimationFrame(frame);
			observer.disconnect();
		};
	}, [fitColumn, minimumWidth, offered]);
	// Over the terminal area the File Bar follows that area, which it opens from.
	const [overlayPlace, setOverlayPlace] = React.useState<FileBarOverlayPlace | null>(null);
	React.useLayoutEffect(() => {
		const area = areaReference.current;
		const root = rootReference.current;
		if (!overlayShown || mobile || area === null || root === null) return undefined;
		const measure = (): void => {
			const box = area.getBoundingClientRect();
			const start = Math.round(box.left - root.getBoundingClientRect().left);
			const across = layout.width >= box.width;
			setOverlayPlace((current) =>
				current?.start === start && current.across === across ? current : { across, start },
			);
		};
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(area);
		observer.observe(root);
		return () => observer.disconnect();
	}, [layout.width, mobile, overlayShown, rootReference]);
	// Filling the window, it covers the terminal, which never takes the keys beneath it, so the
	// terminal taking them closes it, as it closes the lists that fill the window.
	React.useEffect(() => {
		const root = rootReference.current;
		if (!overlayShown || !mobile || root === null) return undefined;
		const onFocusIn = (event: FocusEvent): void => {
			if (resolveWorkbenchFocusRegionFromTarget(event.target) === 'terminal') setOverlayOpen(false);
		};
		root.addEventListener('focusin', onFocusIn);
		return () => root.removeEventListener('focusin', onFocusIn);
	}, [mobile, overlayShown, rootReference]);
	// The File Bar mounts afresh wherever it shows next, docked on either side or over the terminal
	// area, so leaving a place drops its focus request, which would otherwise take focus there; only a
	// request made as it shows from hiding survives.
	let placement: FileBarPlacement = 'hidden';
	if (docked) placement = position;
	else if (overlayShown) placement = 'overlay';
	const [placementForFocus, setPlacementForFocus] = React.useState(placement);
	if (placementForFocus !== placement) {
		if (placementForFocus !== 'hidden') setViewFocusRequest(0);
		setPlacementForFocus(placement);
	}
	// Keys the File Bar held as it left its place go to its active view where it shows next, or to the
	// terminal while it shows nowhere, unless a file's tab or a surface over the content took them.
	const placed = React.useRef(placement);
	React.useLayoutEffect(() => {
		if (placed.current === placement) return;
		placed.current = placement;
		const focusLeft = focusLeftWithColumn.current && !keysWentToFile.current;
		focusLeftWithColumn.current = false;
		keysWentToFile.current = false;
		if (!focusLeft) return;
		if (placement !== 'hidden') setViewFocusRequest((request) => request + 1);
		else if (!surfacesOverContent) focusTerminal();
	}, [focusTerminal, placement, surfacesOverContent]);
	// The shown File Bar takes its place in the region cycle, where its active view takes the keys.
	React.useEffect(() => {
		if (!shown) return undefined;
		return supplyFocusRegion('file-bar', () => setViewFocusRequest((request) => request + 1));
	}, [shown, supplyFocusRegion]);
	// The Workspaces the File Bar has shown in this run, whose sessions it keeps for the count badge.
	const [shownWorkspaces, setShownWorkspaces] = React.useState<ReadonlySet<string>>(NO_WORKSPACES);
	if (shown && workspaceId !== null && !shownWorkspaces.has(workspaceId)) {
		setShownWorkspaces(new Set([...shownWorkspaces, workspaceId]));
	}
	const holding = offered && workspaceId !== null && (shown || shownWorkspaces.has(workspaceId));
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
		if (!shown || load.kind !== 'idle') return undefined;
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
	}, [shown, load.kind]);
	// Dropping the toggles row on the other side of the terminal area moves the File Bar there.
	const moveByDrag = useMemoizedFn((target: FileBarPosition) => {
		reportActionOutcome('pointer', () =>
			typedRuntime.execute(
				target === 'left'
					? WORKBENCH_ACTION_IDS.moveFileBarLeft
					: WORKBENCH_ACTION_IDS.moveFileBarRight,
				undefined,
				{ scope: actionScope, source: 'pointer' },
			),
		);
	});
	const moveDrag = useWorkbenchFileBarMoveDrag({
		areaReference,
		elementReference,
		label: intl.formatMessage(fileBarMessages.label),
		move: moveByDrag,
		position,
		rootReference,
	});
	const toggleView = offered
		? {
				badge:
					shown || session === null || load.kind !== 'ready'
						? null
						: { Badge: load.module.FileBarToggleBadge, session },
				label: intl.formatMessage(
					shown ? fileBarMessages.hideFileBar : fileBarMessages.showFileBar,
				),
				onToggle: invokeToggle,
				shown,
			}
		: null;
	const base = {
		areaReference,
		fileOpened,
		moveCatcher: moveDrag.catcher,
		onLayoutChanged,
		position,
		toggle: toggleView,
	};
	const content = (): WorkbenchFileBarContent => ({
		activeView: layout.activeView,
		attachElement,
		content: load.kind === 'ready' ? load.module.FileBarColumn : null,
		failure: load.kind === 'failed' ? intl.formatMessage(fileBarMessages.loadFailed) : null,
		label: intl.formatMessage(fileBarMessages.label),
		onSelectView: selectView,
		rowMenu,
		session,
		toggles,
		viewFocusRequest,
	});
	if (docked) {
		return {
			...base,
			column: {
				...content(),
				defaultSize: `${layout.width}px`,
				id: FILE_BAR_PANEL_ID,
				maxSize: `${FILE_BAR_WIDTH.maximum}px`,
				minSize: `${minimumWidth}px`,
				onDividerDoubleClick,
				onDividerKeyDown,
				onResize,
				onRowPointerDown: move === undefined ? null : moveDrag.onRowPointerDown,
				panelReference,
				terminalAreaMinSize: `${TERMINAL_AREA_MINIMUM_WIDTH}px`,
			},
			overlay: null,
		};
	}
	if (!overlayShown) return { ...base, column: null, overlay: null };
	const overlayContent = {
		...content(),
		onKeyDown: onOverlayKeyDown,
		onScrimPointerDown: onOverlayScrimPointerDown,
		width: layout.width,
	};
	// Filling the window, it touches both of the window's ends.
	if (mobile) {
		return {
			...base,
			column: null,
			overlay: {
				...overlayContent,
				atWindowEnd: true,
				atWindowStart: true,
				fill: true,
				inlineStart: 0,
			},
		};
	}
	if (overlayPlace === null) return { ...base, column: null, overlay: null };
	// Reaching across the terminal area, it touches both of the area's ends.
	const { across, start } = overlayPlace;
	return {
		...base,
		column: null,
		overlay: {
			...overlayContent,
			atWindowEnd: position === 'right' || across,
			atWindowStart: start === 0 && (position === 'left' || across),
			fill: false,
			inlineStart: start,
		},
	};
}
