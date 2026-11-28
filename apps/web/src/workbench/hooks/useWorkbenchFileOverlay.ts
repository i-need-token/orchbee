import {
	createTypedActionRuntime,
	type ActionExecution,
	type ActionInvocationSource,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileOverlayMessages, useOrchbeeIntl, type MessageDescriptor } from '@orchbee/i18n';
import * as React from 'react';

import {
	FileOverlayRegistry,
	loadFileOverlay,
	splitFileTabPath,
	type FileOverlayCommandSource,
	type FileOverlayCommands,
	type FileOverlayEvent,
	type FileOverlayFocus,
	type FileOverlayMenuEntry,
	type FileOverlayModule,
	type FileOverlayRegionElements,
	type FileOverlayShortcutLabels,
	type FileOverlayStore,
	type FileOverlayTitleActions,
} from '../../file-overlay';
import type { PointerDropTarget } from '../../pointer-drag';
import type { WorkbenchWorkspace } from '../../project-workspace';
import { registerWorkbenchFileOverlayActions } from '../actions/registerWorkbenchFileOverlayActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../actions/workbench-action-ids';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import type { WorkbenchNotificationQueue } from '../components/workbench-notification-cards/model/WorkbenchNotificationQueue';
import type { WorkbenchQuickInputSessions } from '../components/workbench-search-surface/model/WorkbenchQuickInputSessions';
import type { WorkbenchTerminalContext } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchTerminalPathDrops } from '../model/WorkbenchTerminalPathDrops';
import { buildFileOverlayTitleActions, buildFileTabMenuEntries } from '../utils/file-overlay-menus';
import type { WorkbenchAuthorityNoticeView } from './useWorkbenchAuthorityNotice';

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
	/** The quick input, where Show Editors By Most Recently Used lists the tabs. */
	readonly quickInputSessions: Pick<WorkbenchQuickInputSessions, 'pick'>;
	readonly reportActionOutcome: WorkbenchAuthorityNoticeView['reportActionOutcome'];
	/** The Workbench root, which places the overlay. */
	readonly rootReference: React.RefObject<HTMLElement | null>;
	/** The keys that run an Action now, for the overlay's tooltips. */
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	/** The platform whose pointer and key conventions the overlay follows. */
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	/** Whether the Sidebar shows docked, which offers left-expand while the File Bar is on the right. */
	readonly sidebarDocked: boolean;
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
	readonly platform: WorkbenchShortcutPlatform;
	/** Receives how the Workbench gives the overlay the keys. */
	readonly reference: React.RefObject<FileOverlayFocus | null>;
	readonly regions: FileOverlayRegionElements;
	readonly scrimDrop: PointerDropTarget | null;
	readonly shortcutLabels: FileOverlayShortcutLabels;
	readonly store: FileOverlayStore;
}

interface WorkbenchFileOverlayView {
	/** Present while the shown Workspace's overlay is not closed and its chunk has loaded. */
	readonly host: WorkbenchFileOverlayHost | null;
	/** The Sidebar's panel, whose box left-expand covers. */
	readonly sidebarReference: React.RefObject<HTMLDivElement | null>;
	/** The terminal area, whose box the default tier covers. */
	readonly terminalAreaReference: React.RefObject<HTMLDivElement | null>;
}

const subscribeToNothing = (): (() => void) => () => undefined;
const NO_TITLE_ACTIONS: FileOverlayTitleActions = { buttons: [], more: [] };
const noActiveTab = (): null => null;
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
	focusTerminal,
	notifications,
	offered,
	quickInputSessions,
	reportActionOutcome,
	rootReference,
	shortcutLabelForAction,
	shortcutPlatform,
	sidebarDocked,
	terminalContext,
	terminalPathDrops,
	workspaceId,
	workspaces,
}: UseWorkbenchFileOverlayOptions): WorkbenchFileOverlayView {
	const intl = useOrchbeeIntl();
	const [registry] = React.useState(() =>
		offered ? new FileOverlayRegistry(sidebarDocked) : null,
	);
	const sidebarReference = React.useRef<HTMLDivElement>(null);
	const terminalAreaReference = React.useRef<HTMLDivElement>(null);
	const overlayReference = React.useRef<FileOverlayFocus>(null);
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
	const dispatch = useMemoizedFn((event: FileOverlayEvent) => {
		if (store === null) return;
		for (const effect of store.dispatch(event)) {
			// Saving and reverting answer a close's save question, which no close asks yet.
			if (effect.kind === 'focus-terminal') focusTerminal();
		}
	});
	// The overlay keeps one function across renders, whichever the Workbench passes.
	const returnToTerminal = useMemoizedFn(() => focusTerminal());
	const resolveCurrentTerminal = useMemoizedFn(() => currentTerminal());
	// Typing into the terminal replaces what the overlay covered, so the overlay shrinks out of its way.
	const scrimDrop = React.useMemo(
		() =>
			terminalPathDrops?.currentTerminalTarget(resolveCurrentTerminal, () =>
				dispatch({ type: 'terminal-replaced' }),
			) ?? null,
		[dispatch, resolveCurrentTerminal, terminalPathDrops],
	);
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
			closeTab: (tabId, source) =>
				invokeTabAction(WORKBENCH_ACTION_IDS.closeFileOverlayTab, tabId, source),
			keepTab: (tabId, source) =>
				invokeTabAction(WORKBENCH_ACTION_IDS.keepFileOverlayTab, tabId, source),
			selectTab: (tabId, source) =>
				invokeTabAction(WORKBENCH_ACTION_IDS.selectFileOverlayTab, tabId, source),
			tabMenu,
			titleActions,
			toggleFull: invokeToggleFull,
			unpinTab: (tabId, source) =>
				invokeTabAction(WORKBENCH_ACTION_IDS.unpinFileOverlayTab, tabId, source),
		}),
		[invokeTabAction, invokeToggleFull, tabMenu, titleActions],
	);
	// VS Code's editor picker: each tab by its file's name, icon and folder, the preview tab in italics.
	const pickTab = useMemoizedFn(async (signal: AbortSignal): Promise<string | null> => {
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
		const outcome = await quickInputSessions.pick({
			entries,
			matchOnDescription: true,
			placeholder: intl.formatMessage(fileOverlayMessages.tabPickPlaceholder),
			signal,
		});
		return outcome.kind === 'accepted' ? outcome.value : null;
	});
	const focusOverlay = useMemoizedFn(() => overlayReference.current?.focus());
	React.useEffect(() => {
		registry?.showTerminal(
			workspaceId === null ? null : { tabId: activeTabId, terminalContextId, workspaceId },
		);
	}, [activeTabId, registry, terminalContextId, workspaceId]);
	React.useEffect(() => {
		registry?.setLeftExpandOffered(sidebarDocked);
	}, [registry, sidebarDocked]);
	React.useEffect(() => {
		registry?.retain(new Set(workspaces.map((workspace) => workspace.id)));
	}, [registry, workspaces]);
	React.useEffect(() => {
		if (store === null || workspaceId === null) return undefined;
		const registrations = registerWorkbenchFileOverlayActions({
			dispatch,
			focusOverlay,
			getState: () => store.state.getState(),
			pickTab,
			scope: actionScope,
			typedRuntime,
			workspaceId,
		});
		return () => {
			for (const registration of registrations) registration.dispose();
		};
	}, [actionScope, dispatch, focusOverlay, pickTab, store, typedRuntime, workspaceId]);
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
				setLoad({ kind: 'failed' });
				reportLoadFailure();
			},
		);
		return () => {
			current = false;
		};
	}, [load.kind, reportLoadFailure, shown]);
	const toggleFullShortcutLabel = shortcutLabelForAction(
		WORKBENCH_ACTION_IDS.toggleFileOverlayFull,
	);
	const shortcutLabels = React.useMemo<FileOverlayShortcutLabels>(
		() => ({ toggleFull: toggleFullShortcutLabel }),
		[toggleFullShortcutLabel],
	);
	const host =
		store === null || !shown || load.kind !== 'ready'
			? null
			: {
					commands,
					content: load.module.FileOverlayHost,
					dispatch,
					focusTerminal: returnToTerminal,
					platform: shortcutPlatform,
					reference: overlayReference,
					regions,
					scrimDrop,
					shortcutLabels,
					store,
				};
	return { host, sidebarReference, terminalAreaReference };
}
