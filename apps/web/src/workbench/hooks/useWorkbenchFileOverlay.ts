import {
	createTypedActionRuntime,
	type ActionExecution,
	type ActionInvocationSource,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileOverlayMessages, useOrchbeeIntl, type MessageDescriptor } from '@orchbee/i18n';
import { keyboardShortcutChordModifiers } from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import {
	dispatchFileOverlayEvent,
	FILE_OVERLAY_ACTION_IDS,
	FileOverlayRegistry,
	loadFileOverlay,
	readFileOverlayPipPosition,
	splitFileTabPath,
	writeFileOverlayPipPosition,
	type FileOverlayCommandSource,
	type FileOverlayCommands,
	type FileOverlayEvent,
	type FileOverlayFocus,
	type FileOverlayMenuEntry,
	type FileOverlayModule,
	type FileOverlayOpenFile,
	type FileOverlayPipPosition,
	type FileOverlayPipStorage,
	type FileOverlayRegionElements,
	type FileOverlayShortcutLabels,
	type FileOverlayState,
	type FileOverlayStore,
	type FileOverlayTitleActions,
	type FileTabContentPort,
} from '../../file-overlay';
import { isInFileBarTogglesRow, type FileBarPosition } from '../../file-bar';
import type { FileResource } from '../../file-session';
import type { PointerDropTarget } from '../../pointer-drag';
import type { WorkbenchWorkspace } from '../../project-workspace';
import {
	registerWorkbenchFileOverlayActions,
	type FileTabPickOptions,
	type FileTabPicker,
} from '../actions/registerWorkbenchFileOverlayActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../actions/workbench-action-ids';
import type { WorkbenchFocusRegion } from '../model/resolveWorkbenchFocusTarget';
import type { WorkbenchFocusRouting } from '../types/workbench-focus-routing';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import type { WorkbenchNotificationQueue } from '../components/workbench-notification-cards/model/WorkbenchNotificationQueue';
import type {
	WorkbenchQuickInputSessions,
	WorkbenchQuickNavigateKeys,
} from '../components/workbench-search-surface/model/WorkbenchQuickInputSessions';
import type { WorkbenchTerminalContext } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchTerminalPathDrops } from '../model/WorkbenchTerminalPathDrops';
import { buildFileOverlayTitleActions, buildFileTabMenuEntries } from '../utils/file-overlay-menus';
import { resolveWorkbenchFocusRegionFromTarget } from '../utils/workbench-focus-dom';
import type { WorkbenchScopedShortcutBinding } from '../utils/workbench-shortcuts';
import type { WorkbenchAuthorityNoticeView } from './useWorkbenchAuthorityNotice';
import { useWorkbenchFileOverlayReturnFocus } from './useWorkbenchFileOverlayReturnFocus';

type FileOverlayLoad =
	| { readonly kind: 'failed' }
	| { readonly kind: 'idle' }
	| { readonly kind: 'ready'; readonly module: FileOverlayModule };

const IDLE_LOAD: FileOverlayLoad = { kind: 'idle' };

interface UseWorkbenchFileOverlayOptions {
	readonly actionRuntime: ActionRuntime;
	/** The Workbench's own scope, where the overlay's Actions resolve. */
	readonly actionScope: ActionScope;
	/** The terminal a press on the scrim gives the keys to, which the scrim covers. */
	readonly currentTerminal: () => string | null;
	/** Gives keyboard focus back to the current terminal. */
	readonly focusTerminal: () => void;
	/** Where a failed load of the overlay's chunk is reported. */
	readonly notifications: Pick<WorkbenchNotificationQueue, 'notify'>;
	/** Whether this window offers the File Bar; without the offer no overlay exists or listens. */
	readonly offered: boolean;
	/** The quick input, where the tab picker lists the tabs. */
	readonly quickInputSessions: Pick<WorkbenchQuickInputSessions, 'navigate' | 'pick'>;
	readonly reportActionOutcome: WorkbenchAuthorityNoticeView['reportActionOutcome'];
	/** The Workbench root, which places the overlay. */
	readonly rootReference: React.RefObject<HTMLElement | null>;
	/** The File Bar family's keys now, whose modifier keys the tab picker's quick navigate waits on. */
	readonly scopedShortcutBindings: readonly WorkbenchScopedShortcutBinding[] | undefined;
	/** The keys that run an Action now, for the overlay's tooltips. */
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	/** The platform whose pointer and key conventions the overlay follows. */
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	/** Whether the Sidebar shows docked, which offers left-expand while the File Bar is on the right. */
	readonly sidebarDocked: boolean;
	/**
	 * The File Bar's side of the terminal area: on the left it lies between the Sidebar and the
	 * terminal area, where left-expand would cover it, so left-expand is not offered.
	 */
	readonly fileBarPosition: FileBarPosition;
	/**
	 * Where picture-in-picture's position persists, read once as the Workbench starts, and each
	 * Workspace's tabs, read as its overlay starts.
	 */
	readonly storage: FileOverlayPipStorage | undefined;
	/** Adds the shown overlay to the region cycle. */
	readonly supplyFocusRegion: WorkbenchFocusRouting['supplyRegion'];
	/**
	 * The content behind the tabs, which saves and reverts them for the save question and hears every
	 * close; absent while no view shows their content, so no tab can have unsaved edits.
	 */
	readonly tabContent?: FileTabContentPort | undefined;
	/** The active Workspace's terminals, which the terminal area shows; null outside a Workspace. */
	readonly terminalContext: WorkbenchTerminalContext | null;
	/** Drops of files on terminals, which the scrim passes to the terminal it covers. */
	readonly terminalPathDrops: WorkbenchTerminalPathDrops | undefined;
	/** The active Workspace; null outside a Workspace. */
	readonly workspaceId: string | null;
	/** The window's Workspaces, whose overlays live while they exist. */
	readonly workspaces: readonly WorkbenchWorkspace[];
}

/** The shown Workspace's overlay, once its chunk has loaded. */
interface WorkbenchFileOverlayHost {
	readonly commands: FileOverlayCommands;
	readonly content: FileOverlayModule['FileOverlayHost'];
	readonly dispatch: (event: FileOverlayEvent) => void;
	readonly focusTerminal: () => void;
	readonly movePip: (position: FileOverlayPipPosition) => void;
	readonly pipPosition: FileOverlayPipPosition | null;
	readonly platform: WorkbenchShortcutPlatform;
	/** Receives how the Workbench gives the overlay the keys. */
	readonly reference: React.RefCallback<FileOverlayFocus>;
	readonly regions: FileOverlayRegionElements;
	readonly returnFocus: () => void;
	readonly scrimDrop: PointerDropTarget | null;
	readonly shortcutLabels: FileOverlayShortcutLabels;
	readonly store: FileOverlayStore;
}

interface WorkbenchFileOverlayView {
	/** The file the shown overlay's active tab shows, as VS Code's active editor; null without tabs. */
	readonly activeFile: FileResource | null;
	/** Present while the shown Workspace's overlay is not closed and its chunk has loaded. */
	readonly host: WorkbenchFileOverlayHost | null;
	/** Opens a file in the shown Workspace's overlay, for the File Bar's views and terminal links. */
	readonly openFile: FileOverlayOpenFile;
	/** The Sidebar's panel, whose box left-expand covers. */
	readonly sidebarReference: React.RefObject<HTMLDivElement | null>;
	/** The terminal area, whose box the default tier covers. */
	readonly terminalAreaReference: React.RefObject<HTMLDivElement | null>;
	/**
	 * Called as a File Bar control in the 36px row acts: the shown overlay leaves the full tier for the
	 * tier it had before, so the File Bar it acts on shows.
	 */
	readonly uncoverFileBar: () => void;
}

/**
 * Whether the open overlay lies over a region at its tier. Every tier covers the terminal area and
 * none the 36px row the Tab Strip is in; left-expand adds the docked Sidebar, and full adds the File
 * Bar too. A Sidebar that is not docked opens over the terminal area, so every tier covers it.
 */
function coversFocusRegion(
	tier: FileOverlayState['mode']['tier'],
	region: WorkbenchFocusRegion,
	sidebarDocked: boolean,
): boolean {
	switch (region) {
		case 'terminal':
			return true;
		case 'sidebar':
			return !sidebarDocked || tier !== 'default';
		case 'file-bar':
			return tier === 'full';
		case 'file-overlay':
		case 'tab-strip':
			return false;
	}
}

/** Without a view of the tabs' content no tab gains unsaved edits, so no save is ever asked of it. */
const NO_TAB_CONTENT: FileTabContentPort = {
	closed: () => undefined,
	revert: () => undefined,
	save: (tabs) => Promise.resolve(tabs.map((tab) => tab.id)),
};

const subscribeToNothing = (): (() => void) => () => undefined;
const NO_TITLE_ACTIONS: FileOverlayTitleActions = { buttons: [], more: [] };
const noActiveTab = (): null => null;
const noActiveFile = (): null => null;
const neverShown = (): boolean => false;

/** The overlay's Actions that name one tab. */
type FileOverlayTabActionId =
	| typeof WORKBENCH_ACTION_IDS.closeFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.keepFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.selectFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.unpinFileOverlayTab;

/**
 * Keeps one file overlay per Workspace for this Workbench and tells the overlays what the Workbench
 * changes beneath them: the terminal the active Workspace shows, whether left-expand is offered, and
 * which Workspaces still exist. The active Workspace's overlay shows from its own chunk, which loads
 * the first time an overlay is not closed.
 */
export function useWorkbenchFileOverlay({
	actionRuntime,
	actionScope,
	currentTerminal,
	fileBarPosition,
	focusTerminal,
	notifications,
	offered,
	quickInputSessions,
	reportActionOutcome,
	rootReference,
	scopedShortcutBindings,
	shortcutLabelForAction,
	shortcutPlatform,
	sidebarDocked,
	storage,
	supplyFocusRegion,
	tabContent = NO_TAB_CONTENT,
	terminalContext,
	terminalPathDrops,
	workspaceId,
	workspaces,
}: UseWorkbenchFileOverlayOptions): WorkbenchFileOverlayView {
	const intl = useOrchbeeIntl();
	const leftExpandOffered = sidebarDocked && fileBarPosition === 'right';
	const [registry] = React.useState(() =>
		offered ? new FileOverlayRegistry(leftExpandOffered, storage) : null,
	);
	// Every Workspace's picture-in-picture shows where a drag last left one.
	const [pipPosition, setPipPosition] = React.useState<FileOverlayPipPosition | null>(() =>
		offered ? readFileOverlayPipPosition(storage) : null,
	);
	const movePip = useMemoizedFn((position: FileOverlayPipPosition) => {
		setPipPosition(position);
		writeFileOverlayPipPosition(storage, position);
	});
	const sidebarReference = React.useRef<HTMLDivElement>(null);
	const terminalAreaReference = React.useRef<HTMLDivElement>(null);
	const overlayReference = React.useRef<FileOverlayFocus>(null);
	// The overlay mounts once the change that shows it renders, after the Action that asked for the
	// keys with that change has returned; until then the ask waits.
	const focusWaiting = React.useRef(false);
	const [regions] = React.useState<FileOverlayRegionElements>(() => ({
		root: rootReference,
		sidebar: sidebarReference,
		terminalArea: terminalAreaReference,
	}));
	const [load, setLoad] = React.useState<FileOverlayLoad>(IDLE_LOAD);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
		[actionRuntime],
	);
	const panels = registry === null ? null : (terminalContext?.panelWorkspaceStore ?? null);
	const activeTabId = React.useSyncExternalStore(
		panels?.subscribe ?? subscribeToNothing,
		panels?.getActiveTabId ?? noActiveTab,
		panels?.getActiveTabId ?? noActiveTab,
	);
	const terminalContextId = terminalContext?.terminalContextId ?? null;
	// The active Workspace's overlay is created the first time anything asks for it.
	const store = registry === null || workspaceId === null ? null : registry.store(workspaceId);
	const isShown = React.useMemo(
		() => (store === null ? neverShown : () => store.state.getState().mode.kind !== 'closed'),
		[store],
	);
	const shown = React.useSyncExternalStore(
		store?.state.subscribe ?? subscribeToNothing,
		isShown,
		isShown,
	);
	const readActiveFile = React.useMemo(
		() =>
			store === null
				? noActiveFile
				: (): FileResource | null => {
						const { tabs } = store.state.getState();
						return tabs.tabs.find((tab) => tab.id === tabs.activeId)?.input.resource ?? null;
					},
		[store],
	);
	const activeFile = React.useSyncExternalStore(
		store?.state.subscribe ?? subscribeToNothing,
		readActiveFile,
		readActiveFile,
	);
	const dispatch = useMemoizedFn((event: FileOverlayEvent) => {
		if (store !== null) {
			dispatchFileOverlayEvent(store, event, { content: tabContent, focusTerminal });
		}
	});
	const uncoverFileBar = useMemoizedFn(() => dispatch({ type: 'file-bar-control-activated' }));
	// The overlay keeps one function across renders, whichever the Workbench passes.
	const returnToTerminal = useMemoizedFn(() => focusTerminal());
	const returnFocus = useWorkbenchFileOverlayReturnFocus(store, returnToTerminal);
	const resolveCurrentTerminal = useMemoizedFn(() => currentTerminal());
	// Typing into the terminal replaces what the overlay covered, so the overlay shrinks out of its way.
	const scrimDrop = React.useMemo(
		() =>
			terminalPathDrops?.currentTerminalTarget(resolveCurrentTerminal, () =>
				dispatch({ type: 'terminal-replaced' }),
			) ?? null,
		[dispatch, resolveCurrentTerminal, terminalPathDrops],
	);
	const openFile: FileOverlayOpenFile = useMemoizedFn((input, invocation) =>
		typedRuntime.execute(FILE_OVERLAY_ACTION_IDS.openFile, input, invocation),
	);
	const invokeCloseOverlay = useMemoizedFn(() => {
		reportActionOutcome('button', () =>
			typedRuntime.execute(WORKBENCH_ACTION_IDS.closeFileOverlay, undefined, {
				scope: actionScope,
				source: 'button',
			}),
		);
	});
	const invokeToggleFull = useMemoizedFn(() => {
		reportActionOutcome('button', () =>
			typedRuntime.execute(WORKBENCH_ACTION_IDS.toggleFileOverlayFull, undefined, {
				scope: actionScope,
				source: 'button',
			}),
		);
	});
	const invokeTabAction = useMemoizedFn(
		(actionId: FileOverlayTabActionId, tabId: string, source: FileOverlayCommandSource) => {
			if (workspaceId === null) return;
			const input = { tabId, workspaceId };
			reportActionOutcome(source, () =>
				typedRuntime.execute(actionId, input, { scope: actionScope, source }),
			);
		},
	);
	const moveTab = useMemoizedFn(
		(tabId: string, index: number, source: FileOverlayCommandSource) => {
			if (workspaceId === null) return;
			const input = { index, tabId, workspaceId };
			reportActionOutcome(source, () =>
				typedRuntime.execute(WORKBENCH_ACTION_IDS.moveFileOverlayTab, input, {
					scope: actionScope,
					source,
				}),
			);
		},
	);
	const menuOptions = (tabId: string, shownWorkspaceId: string) => ({
		formatMessage: (descriptor: MessageDescriptor) => intl.formatMessage(descriptor),
		report: (source: ActionInvocationSource, start: () => ActionExecution<unknown>) => {
			reportActionOutcome(source, start);
		},
		scope: actionScope,
		shortcutLabelForAction,
		tabId,
		typedRuntime,
		workspaceId: shownWorkspaceId,
	});
	const tabMenu = useMemoizedFn((tabId: string): readonly FileOverlayMenuEntry[] =>
		workspaceId === null ? [] : buildFileTabMenuEntries(menuOptions(tabId, workspaceId)),
	);
	const titleActions = useMemoizedFn(
		(tabId: string): FileOverlayTitleActions =>
			workspaceId === null
				? NO_TITLE_ACTIONS
				: buildFileOverlayTitleActions(menuOptions(tabId, workspaceId)),
	);
	const commands = React.useMemo<FileOverlayCommands>(
		() => ({
			closeOverlay: invokeCloseOverlay,
			closeTab: (tabId, source) =>
				invokeTabAction(WORKBENCH_ACTION_IDS.closeFileOverlayTab, tabId, source),
			keepTab: (tabId, source) =>
				invokeTabAction(WORKBENCH_ACTION_IDS.keepFileOverlayTab, tabId, source),
			moveTab,
			selectTab: (tabId, source) =>
				invokeTabAction(WORKBENCH_ACTION_IDS.selectFileOverlayTab, tabId, source),
			tabMenu,
			titleActions,
			toggleFull: invokeToggleFull,
			unpinTab: (tabId, source) =>
				invokeTabAction(WORKBENCH_ACTION_IDS.unpinFileOverlayTab, tabId, source),
		}),
		[invokeCloseOverlay, invokeTabAction, invokeToggleFull, moveTab, tabMenu, titleActions],
	);
	// How many tab picks wait on the quick input; while one does, its current session is the picker.
	const tabPicks = React.useRef(0);
	// VS Code's editor picker: each tab by its file's name, icon and folder, the preview tab in italics.
	const pickTab = useMemoizedFn(async (options: FileTabPickOptions): Promise<string | null> => {
		if (store === null) return null;
		const { tabs } = store.state.getState();
		const entries = tabs.recent.flatMap((id) => {
			const tab = tabs.tabs.find((candidate) => candidate.id === id);
			if (tab === undefined) return [];
			const { path } = tab.input.resource;
			const { folder, name } = splitFileTabPath(path);
			return [
				{
					description: folder,
					icon: { kind: 'file', name, path },
					id,
					italic: id === tabs.previewId,
					kind: 'item',
					label: name,
					value: id,
				} as const,
			];
		});
		tabPicks.current += 1;
		try {
			const outcome = await quickInputSessions.pick({
				...options,
				entries,
				matchOnDescription: true,
				placeholder: intl.formatMessage(fileOverlayMessages.tabPickPlaceholder),
			});
			return outcome.kind === 'accepted' ? outcome.value : null;
		} finally {
			tabPicks.current -= 1;
		}
	});
	const tabPicker = React.useMemo<FileTabPicker>(
		() => ({
			navigate: (direction, quickNavigate) => quickInputSessions.navigate(direction, quickNavigate),
			pick: pickTab,
			showing: () => tabPicks.current > 0,
		}),
		[pickTab, quickInputSessions],
	);
	// VS Code's quick navigate waits on the modifier keys of every one-chord binding of its Action.
	const quickNavigateKeys = useMemoizedFn(
		(actionId: string): readonly WorkbenchQuickNavigateKeys[] =>
			(scopedShortcutBindings ?? []).flatMap(({ definition, sequence }) =>
				definition.actionId === actionId && sequence.length === 1
					? [keyboardShortcutChordModifiers(sequence[0], shortcutPlatform)]
					: [],
			),
	);
	const focusOverlay = useMemoizedFn((focus: boolean) => {
		const overlay = overlayReference.current;
		focusWaiting.current = focus && overlay === null;
		if (focus) overlay?.focus();
	});
	const attachOverlay = useMemoizedFn((overlay: FileOverlayFocus | null) => {
		overlayReference.current = overlay;
		if (overlay === null || !focusWaiting.current) return;
		focusWaiting.current = false;
		overlay.focus();
	});
	React.useEffect(() => {
		registry?.showTerminal(
			workspaceId === null ? null : { tabId: activeTabId, terminalContextId, workspaceId },
		);
	}, [activeTabId, registry, terminalContextId, workspaceId]);
	React.useEffect(() => {
		registry?.setLeftExpandOffered(leftExpandOffered);
	}, [leftExpandOffered, registry]);
	React.useEffect(() => {
		registry?.retain(new Set(workspaces.map((workspace) => workspace.id)));
	}, [registry, workspaces]);
	// The shown overlay takes its place in the region cycle after the File Bar: open, its active tab
	// takes the keys; shrunk, picture-in-picture does.
	React.useEffect(() => {
		if (!shown) return undefined;
		return supplyFocusRegion('file-overlay', () => focusOverlay(true));
	}, [focusOverlay, shown, supplyFocusRegion]);
	// Focus moving into a region the open overlay covers replaces what it covered, as region cycling
	// into the terminal does: the overlay shrinks out of the way and the keys stay where they went.
	// The File Bar's toggles row stays above every tier, so the keys reaching it replace nothing.
	React.useEffect(() => {
		const root = rootReference.current;
		if (store === null || root === null) return undefined;
		const onFocusIn = (event: FocusEvent): void => {
			const { mode } = store.state.getState();
			if (mode.kind !== 'open' || isInFileBarTogglesRow(event.target)) return;
			const region = resolveWorkbenchFocusRegionFromTarget(event.target);
			if (region !== null && coversFocusRegion(mode.tier, region, sidebarDocked)) {
				dispatch({ type: 'terminal-replaced' });
			}
		};
		root.addEventListener('focusin', onFocusIn);
		return () => root.removeEventListener('focusin', onFocusIn);
	}, [dispatch, rootReference, sidebarDocked, store]);
	React.useEffect(() => {
		if (store === null || workspaceId === null) return undefined;
		const registrations = registerWorkbenchFileOverlayActions({
			dispatch,
			focusOverlay,
			getState: () => store.state.getState(),
			quickNavigateKeys,
			scope: actionScope,
			tabPicker,
			typedRuntime,
			workspaceId,
		});
		return () => {
			for (const registration of registrations) registration.dispose();
		};
	}, [
		actionScope,
		dispatch,
		focusOverlay,
		quickNavigateKeys,
		store,
		tabPicker,
		typedRuntime,
		workspaceId,
	]);
	// A failed load says so on a card, and tries again from its button or the next time an overlay
	// shows.
	if (!shown && load.kind === 'failed') setLoad(IDLE_LOAD);
	const reportLoadFailure = useMemoizedFn(() => {
		void notifications
			.notify({
				buttons: [intl.formatMessage(fileOverlayMessages.retry)],
				message: intl.formatMessage(fileOverlayMessages.loadFailed),
				severity: 'error',
			})
			.then((outcome) => {
				if (outcome.kind === 'chosen') setLoad(IDLE_LOAD);
			});
	});
	React.useEffect(() => {
		if (!shown || load.kind !== 'idle') return undefined;
		let current = true;
		void loadFileOverlay().then(
			(module) => {
				if (current) setLoad({ kind: 'ready', module });
			},
			() => {
				if (!current) return;
				// The keys stay where they are, so a later load cannot take them by surprise.
				focusWaiting.current = false;
				setLoad({ kind: 'failed' });
				reportLoadFailure();
			},
		);
		return () => {
			current = false;
		};
	}, [load.kind, reportLoadFailure, shown]);
	const closeOverlayShortcutLabel = shortcutLabelForAction(WORKBENCH_ACTION_IDS.closeFileOverlay);
	const toggleFullShortcutLabel = shortcutLabelForAction(
		WORKBENCH_ACTION_IDS.toggleFileOverlayFull,
	);
	const shortcutLabels = React.useMemo<FileOverlayShortcutLabels>(
		() => ({ closeOverlay: closeOverlayShortcutLabel, toggleFull: toggleFullShortcutLabel }),
		[closeOverlayShortcutLabel, toggleFullShortcutLabel],
	);
	const host =
		store === null || !shown || load.kind !== 'ready'
			? null
			: {
					commands,
					content: load.module.FileOverlayHost,
					dispatch,
					focusTerminal: returnToTerminal,
					movePip,
					pipPosition,
					platform: shortcutPlatform,
					reference: attachOverlay,
					regions,
					returnFocus,
					scrimDrop,
					shortcutLabels,
					store,
				};
	return { activeFile, host, openFile, sidebarReference, terminalAreaReference, uncoverFileBar };
}
