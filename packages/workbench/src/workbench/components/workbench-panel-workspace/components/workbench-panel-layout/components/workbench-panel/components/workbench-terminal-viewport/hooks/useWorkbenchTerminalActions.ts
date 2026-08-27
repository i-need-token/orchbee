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
	readonly entries: readonly WorkbenchActionMenuCommand[];
	readonly scope: ActionScope;
}

const terminalActionOrder = [
	TERMINAL_ACTION_IDS.openLink,
	TERMINAL_ACTION_IDS.copySelection,
	TERMINAL_ACTION_IDS.paste,
	TERMINAL_ACTION_IDS.selectAll,
	TERMINAL_ACTION_IDS.findOpen,
	TERMINAL_ACTION_IDS.hintsOpen,
	TERMINAL_ACTION_IDS.outputShowLast,
	TERMINAL_ACTION_IDS.clear,
] as const;

const terminalActionIcons: Readonly<Record<(typeof terminalActionOrder)[number], ProductIconName>> =
	{
		[TERMINAL_ACTION_IDS.clear]: 'trash',
		[TERMINAL_ACTION_IDS.copySelection]: 'copy',
		[TERMINAL_ACTION_IDS.findOpen]: 'search',
		[TERMINAL_ACTION_IDS.hintsOpen]: 'action',
		[TERMINAL_ACTION_IDS.openLink]: 'action',
		[TERMINAL_ACTION_IDS.outputShowLast]: 'action',
		[TERMINAL_ACTION_IDS.paste]: 'clipboard',
		[TERMINAL_ACTION_IDS.selectAll]: 'check',
	};

export function useWorkbenchTerminalActions({
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
	const scope = React.useMemo(
		() => runtime.createScope(`workbench.terminal:${presentationId}`, parentScope),
		[parentScope, presentationId, runtime],
	);

	React.useEffect(() => {
		const subscription = runtime.subscribe(incrementRuntimeRevision);
		return () => subscription.dispose();
	}, [runtime]);

	const bindActionContextProvider = useMemoizedFn(
		(nextProvider: WorkbenchTerminalActionContextProvider): ActionDisposable => {
			provider.current = nextProvider;
			incrementRuntimeRevision();
			return {
				dispose: () => {
					if (provider.current !== nextProvider) return;
					provider.current = null;
					setPinnedContext(null);
					incrementRuntimeRevision();
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

	const entries = React.useMemo<readonly WorkbenchActionMenuCommand[]>(() => {
		void runtimeRevision;
		if (provider.current === null) return [];
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
		return terminalActionOrder.flatMap((actionId) => {
			const result = readyById.get(actionId);
			if (result === undefined) return [];
			const shortcut = shortcutLabelForAction(actionId);
			return [
				{
					disabled: result.availability.kind !== 'enabled',
					icon: terminalActionIcons[actionId],
					id: `terminal-action:${actionId}`,
					kind: 'command' as const,
					label: formatActionMessage(result.descriptor.title),
					onSelect: () => executePrepared(result.invocation),
					...(shortcut === undefined ? {} : { shortcut }),
				},
			];
		});

		function executePrepared(invocation: PreparedActionInvocation): void {
			const activeProvider = provider.current;
			if (activeProvider === null) {
				void runtime.executePrepared(invocation).completion;
				return;
			}
			void activeProvider.executePrepared(invocation).completion;
		}

		function formatActionMessage(message: ActionMessageDescriptor): string {
			const descriptor: MessageDescriptor = {
				...(message.defaultMessage === undefined ? {} : { defaultMessage: message.defaultMessage }),
				...(message.description === undefined ? {} : { description: message.description }),
				...(message.id === undefined ? {} : { id: message.id }),
			};
			return intl.formatMessage(descriptor, message.values);
		}
	}, [intl, pinnedContext, runtime, runtimeRevision, scope, shortcutLabelForAction]);

	return { bindActionContextProvider, captureContext, entries, scope };
}
