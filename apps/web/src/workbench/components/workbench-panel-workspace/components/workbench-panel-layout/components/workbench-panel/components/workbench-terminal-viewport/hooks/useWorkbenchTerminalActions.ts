import type {
	ActionDisposable,
	ActionMessageDescriptor,
	ActionRuntime,
	ActionScope,
	PreparedActionInvocation,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, type MessageDescriptor } from '@orchbee/i18n';
import type { ProductIconName } from '@orchbee/ui/components';
import * as React from 'react';

import { TERMINAL_ACTION_IDS } from '../../../../../../../../../actions/terminal-action-contracts';
import type { WorkbenchActionMenuCommand } from '../../../../../../../../../types/workbench-action-menu';
import type {
	WorkbenchTerminalActionContextCapture,
	WorkbenchTerminalActionContextProvider,
} from '../../../../../../../../../types/workbench-terminal-surface';

interface UseWorkbenchTerminalActionsOptions {
	readonly onFindOpened: () => void;
	readonly parentScope: ActionScope;
	readonly presentationId: string;
	readonly runtime: ActionRuntime;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
}

interface PinnedTerminalActionContext {
	readonly capture: Extract<WorkbenchTerminalActionContextCapture, { readonly kind: 'product' }>;
}

interface WorkbenchTerminalActions {
	readonly bindActionContextProvider: (
		provider: WorkbenchTerminalActionContextProvider,
	) => ActionDisposable;
	readonly captureContext: (event: React.MouseEvent<HTMLElement>) => boolean;
	/** Empty while the context menu is closed. */
	readonly entries: readonly WorkbenchActionMenuCommand[];
	/** The context menu's open and close-complete events; entries are discovered only between them. */
	readonly onMenuOpenChange: (open: boolean) => void;
	readonly onMenuOpenChangeComplete: (open: boolean) => void;
	readonly scope: ActionScope;
	/** Offered only while another Client holds the terminal; the panel section places it. */
	readonly takeControl: WorkbenchActionMenuCommand | undefined;
}

const terminalActionOrder = [
	TERMINAL_ACTION_IDS.openLink,
	TERMINAL_ACTION_IDS.copySelection,
	TERMINAL_ACTION_IDS.paste,
	TERMINAL_ACTION_IDS.selectAll,
	TERMINAL_ACTION_IDS.findOpen,
	TERMINAL_ACTION_IDS.clear,
] as const;

type TerminalMenuActionId =
	| (typeof terminalActionOrder)[number]
	| typeof TERMINAL_ACTION_IDS.takeControl;

const closedMenuCommands: Pick<WorkbenchTerminalActions, 'entries' | 'takeControl'> = {
	entries: [],
	takeControl: undefined,
};

const terminalActionIcons: Readonly<Record<TerminalMenuActionId, ProductIconName>> = {
	[TERMINAL_ACTION_IDS.clear]: 'clear-all',
	[TERMINAL_ACTION_IDS.takeControl]: 'terminal',
	[TERMINAL_ACTION_IDS.copySelection]: 'copy',
	[TERMINAL_ACTION_IDS.findOpen]: 'search',
	[TERMINAL_ACTION_IDS.openLink]: 'link',
	[TERMINAL_ACTION_IDS.paste]: 'clipboard',
	[TERMINAL_ACTION_IDS.selectAll]: 'select-all',
};

export function useWorkbenchTerminalActions({
	onFindOpened,
	parentScope,
	presentationId,
	runtime,
	shortcutLabelForAction,
}: UseWorkbenchTerminalActionsOptions): WorkbenchTerminalActions {
	const intl = useOrchbeeIntl();
	const provider = React.useRef<WorkbenchTerminalActionContextProvider | null>(null);
	const [pinnedContext, setPinnedContext] = React.useState<PinnedTerminalActionContext | null>(
		null,
	);
	const [runtimeRevision, incrementRuntimeRevision] = React.useReducer(
		(revision: number) => revision + 1,
		0,
	);
	// Discovery reads the availability of every action from this scope to the root, and the runtime
	// changes several times during each Tab or Workspace switch; every mounted terminal discovering
	// on each change held the main thread for tens of milliseconds per switch. Only an open menu
	// shows entries, so only an open menu discovers or follows the runtime.
	const [menuShown, setMenuShown] = React.useState(false);
	const menuShownRef = React.useRef(false);
	const scope = React.useMemo(
		() => runtime.createScope(`workbench.terminal:${presentationId}`, parentScope),
		[parentScope, presentationId, runtime],
	);

	React.useEffect(() => {
		const subscription = runtime.subscribe(() => {
			if (menuShownRef.current) incrementRuntimeRevision();
		});
		return () => subscription.dispose();
	}, [runtime]);

	const onMenuOpenChange = useMemoizedFn((open: boolean): void => {
		if (!open) return;
		menuShownRef.current = true;
		setMenuShown(true);
	});
	// Entries stay until the close animation ends, so a closing menu keeps its items.
	const onMenuOpenChangeComplete = useMemoizedFn((open: boolean): void => {
		if (open) return;
		menuShownRef.current = false;
		setMenuShown(false);
	});

	const bindActionContextProvider = useMemoizedFn(
		(nextProvider: WorkbenchTerminalActionContextProvider): ActionDisposable => {
			provider.current = nextProvider;
			if (menuShownRef.current) incrementRuntimeRevision();
			return {
				dispose: () => {
					if (provider.current !== nextProvider) return;
					provider.current = null;
					setPinnedContext(null);
					if (menuShownRef.current) incrementRuntimeRevision();
				},
			};
		},
	);
	const captureContext = useMemoizedFn((event: React.MouseEvent<HTMLElement>): boolean => {
		const capture = provider.current?.capture({
			altKey: event.altKey,
			clientX: event.clientX,
			clientY: event.clientY,
			ctrlKey: event.ctrlKey,
			metaKey: event.metaKey,
			shiftKey: event.shiftKey,
		});
		if (capture?.kind === 'terminal-owned') {
			setPinnedContext(null);
			return false;
		}
		setPinnedContext(capture === undefined ? null : { capture });
		return true;
	});

	const commands = React.useMemo<Pick<WorkbenchTerminalActions, 'entries' | 'takeControl'>>(() => {
		void runtimeRevision;
		if (!menuShown || provider.current === null) return closedMenuCommands;
		const capture = pinnedContext?.capture;
		const discovered = runtime.discover(
			{
				...(capture?.focus === undefined ? {} : { focus: capture.focus }),
				scope,
				source: 'context-menu',
				...(capture?.target === undefined ? {} : { target: capture.target }),
			},
			capture?.actionInputs === undefined ? {} : { inputByActionId: capture.actionInputs },
		);
		const readyById = new Map(
			discovered
				.filter((result) => result.kind === 'ready')
				.map((result) => [result.descriptor.id, result] as const),
		);
		return {
			entries: terminalActionOrder.flatMap((actionId) => commandFor(actionId) ?? []),
			takeControl: commandFor(TERMINAL_ACTION_IDS.takeControl),
		};

		function commandFor(actionId: TerminalMenuActionId): WorkbenchActionMenuCommand | undefined {
			const result = readyById.get(actionId);
			if (result === undefined) return undefined;
			const shortcut = shortcutLabelForAction(actionId);
			return {
				disabled: result.availability.kind !== 'enabled',
				icon: terminalActionIcons[actionId],
				id: `terminal-action:${actionId}`,
				kind: 'command',
				label: formatActionMessage(result.descriptor.title),
				onSelect: () => executePrepared(result.invocation),
				...(shortcut === undefined ? {} : { shortcut }),
			};
		}

		function executePrepared(invocation: PreparedActionInvocation): void {
			const activeProvider = provider.current;
			const execution =
				activeProvider === null
					? runtime.executePrepared(invocation)
					: activeProvider.executePrepared(invocation);
			if (
				invocation.actionId === TERMINAL_ACTION_IDS.findOpen &&
				execution.getSnapshot().result?.kind === 'completed'
			) {
				onFindOpened();
			}
		}

		function formatActionMessage(message: ActionMessageDescriptor): string {
			const descriptor: MessageDescriptor = {
				...(message.defaultMessage === undefined ? {} : { defaultMessage: message.defaultMessage }),
				...(message.description === undefined ? {} : { description: message.description }),
				...(message.id === undefined ? {} : { id: message.id }),
			};
			return intl.formatMessage(descriptor, message.values);
		}
	}, [
		intl,
		menuShown,
		onFindOpened,
		pinnedContext,
		runtime,
		runtimeRevision,
		scope,
		shortcutLabelForAction,
	]);

	return {
		bindActionContextProvider,
		captureContext,
		entries: commands.entries,
		onMenuOpenChange,
		onMenuOpenChangeComplete,
		scope,
		takeControl: commands.takeControl,
	};
}
