import { useMemoizedFn } from '@orchbee/foundation/react';
import { type MessageDescriptor, useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import type { WorkbenchProjectWorkspaceRuntime } from '../../../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAgentBoard,
	WorkbenchAgentBoardJumpFailure,
	WorkbenchAgentBoardReadFailure,
	WorkbenchAgentBoardSnapshot,
} from '../../../types/workbench-agent-board';
import {
	projectWorkbenchAgentBoardRows,
	projectWorkbenchPresentationNames,
	type WorkbenchAgentBoardRowView,
	type WorkbenchPresentationNames,
} from '../model/project-workbench-agent-board-rows';

interface UseWorkbenchAgentBoardDialogOptions {
	readonly board: WorkbenchAgentBoard;
	/** Called once a jump has shown the session's terminal; the board then closes. */
	readonly onShown: () => void;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
}

interface WorkbenchAgentBoardDialogRow extends WorkbenchAgentBoardRowView {
	readonly elementId: string;
	readonly elementRef: React.RefObject<HTMLDivElement | null> | undefined;
	readonly onClick: () => void;
	readonly onDoubleClick: () => void;
	readonly selected: boolean;
}

interface WorkbenchAgentBoardDialogView {
	readonly activeDescendant: string | undefined;
	readonly canOpen: boolean;
	readonly closeLabel: string;
	/** What the body says in place of rows. */
	readonly emptyMessage: string;
	readonly listLabel: string;
	readonly listRef: React.RefObject<HTMLDivElement | null>;
	readonly onKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
	readonly open: () => void;
	readonly openLabel: string;
	readonly refresh: () => void;
	readonly refreshLabel: string;
	readonly rows: readonly WorkbenchAgentBoardDialogRow[];
	readonly selectLabel: string;
	readonly status: { readonly message: string; readonly warning: boolean } | null;
	readonly title: string;
}

const NO_PRESENTATION_NAMES: ReadonlyMap<string, WorkbenchPresentationNames> = new Map();

const READ_FAILURE_MESSAGES: Readonly<Record<WorkbenchAgentBoardReadFailure, MessageDescriptor>> = {
	denied: workbenchMessages.agentBoardReadDenied,
	transport: workbenchMessages.agentBoardReadTransport,
	unavailable: workbenchMessages.agentBoardReadUnavailable,
};

const JUMP_FAILURE_MESSAGES: Readonly<Record<WorkbenchAgentBoardJumpFailure, MessageDescriptor>> = {
	denied: workbenchMessages.agentBoardJumpDenied,
	ended: workbenchMessages.agentBoardJumpEnded,
	expired: workbenchMessages.agentBoardJumpExpired,
	gone: workbenchMessages.agentBoardJumpGone,
	'no-placement': workbenchMessages.agentBoardJumpNoPlacement,
	unavailable: workbenchMessages.agentBoardJumpUnavailable,
};

/**
 * The open board as the TUI keeps it: an episode for as long as it is mounted, rows in the port's
 * order, a selection that follows its row, and one status line in which a stopped jump
 * outranks a jump under way, which outranks the state of the last read.
 */
export function useWorkbenchAgentBoardDialog({
	board,
	onShown,
	projectWorkspaceRuntime,
}: UseWorkbenchAgentBoardDialogOptions): WorkbenchAgentBoardDialogView {
	const intl = useOrchbeeIntl();
	const elementIdPrefix = React.useId();
	const listRef = React.useRef<HTMLDivElement>(null);
	const selectedElement = React.useRef<HTMLDivElement>(null);
	const snapshot = React.useSyncExternalStore(
		board.subscribe,
		board.getSnapshot,
		board.getSnapshot,
	);
	// Terminal and Tab names change the rows' words without a read.
	const topologyRevision = React.useSyncExternalStore(
		projectWorkspaceRuntime.subscribeTerminalTopology,
		projectWorkspaceRuntime.getTerminalTopologyRevision,
		projectWorkspaceRuntime.getTerminalTopologyRevision,
	);
	const [chosenSessionId, setChosenSessionId] = React.useState<string | null>(null);

	React.useEffect(() => {
		board.open();
		return () => board.close();
	}, [board]);

	const presentationNames = React.useMemo(
		() =>
			// As the sidebar does: the names are read only for the revision this render saw.
			topologyRevision === projectWorkspaceRuntime.getTerminalTopologyRevision()
				? projectWorkbenchPresentationNames(projectWorkspaceRuntime)
				: NO_PRESENTATION_NAMES,
		[projectWorkspaceRuntime, topologyRevision],
	);
	const rowViews = React.useMemo(
		() => projectWorkbenchAgentBoardRows(intl, presentationNames, snapshot.rows),
		[intl, presentationNames, snapshot.rows],
	);
	const selectedIndex = Math.max(
		0,
		rowViews.findIndex((row) => row.sessionId === chosenSessionId),
	);
	const selectedSessionId = rowViews[selectedIndex]?.sessionId ?? null;

	React.useEffect(() => {
		selectedElement.current?.scrollIntoView?.({ block: 'nearest' });
	}, [selectedSessionId]);

	const jump = useMemoizedFn((sessionId: string) => {
		setChosenSessionId(sessionId);
		void board.jump(sessionId).then((outcome) => {
			if (outcome === 'shown') onShown();
		});
	});
	const open = useMemoizedFn(() => {
		if (selectedSessionId !== null) jump(selectedSessionId);
	});
	const refresh = useMemoizedFn(() => board.refresh());
	const select = (index: number): void => {
		const row = rowViews[Math.min(Math.max(index, 0), rowViews.length - 1)];
		if (row !== undefined) setChosenSessionId(row.sessionId);
	};
	const onKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLDivElement>) => {
		if (event.altKey || event.ctrlKey || event.metaKey) return;
		switch (event.key) {
			case 'ArrowDown':
			case 'j':
				select(selectedIndex + 1);
				break;
			case 'ArrowUp':
			case 'k':
				select(selectedIndex - 1);
				break;
			case 'Home':
				select(0);
				break;
			case 'End':
				select(rowViews.length - 1);
				break;
			case 'r':
				refresh();
				break;
			case 'Enter':
				// A focused button answers Enter itself.
				if (event.target !== listRef.current) return;
				open();
				break;
			default:
				return;
		}
		event.preventDefault();
	});

	return {
		activeDescendant:
			selectedSessionId === null ? undefined : `${elementIdPrefix}-${selectedSessionId}`,
		canOpen: selectedSessionId !== null,
		closeLabel: intl.formatMessage(workbenchMessages.resourceClose),
		emptyMessage: intl.formatMessage(emptyMessage(snapshot.read)),
		listLabel: intl.formatMessage(workbenchMessages.agentBoardList),
		listRef,
		onKeyDown,
		open,
		openLabel: intl.formatMessage(workbenchMessages.agentBoardOpen),
		refresh,
		refreshLabel: intl.formatMessage(workbenchMessages.agentBoardRefresh),
		rows: rowViews.map((row) => {
			const selected = row.sessionId === selectedSessionId;
			return {
				...row,
				elementId: `${elementIdPrefix}-${row.sessionId}`,
				elementRef: selected ? selectedElement : undefined,
				onClick: () => setChosenSessionId(row.sessionId),
				onDoubleClick: () => jump(row.sessionId),
				selected,
			};
		}),
		selectLabel: intl.formatMessage(workbenchMessages.agentBoardSelect),
		status: projectStatus(snapshot, (descriptor) => intl.formatMessage(descriptor)),
		title: intl.formatMessage(workbenchMessages.agentBoardTitle),
	};
}

function emptyMessage(read: WorkbenchAgentBoardSnapshot['read']): MessageDescriptor {
	switch (read.kind) {
		case 'failed':
			return READ_FAILURE_MESSAGES[read.reason];
		case 'listed':
			return read.complete
				? workbenchMessages.agentBoardEmpty
				: workbenchMessages.agentBoardEmptyPartial;
		case 'reading':
			return workbenchMessages.agentBoardReadReading;
		case 'unread':
			return workbenchMessages.agentBoardReadUnread;
	}
}

/** The read's state shows here only beside rows; without them the body already says it. */
function projectStatus(
	{ jumping, notice, read, rows }: WorkbenchAgentBoardSnapshot,
	format: (descriptor: MessageDescriptor) => string,
): WorkbenchAgentBoardDialogView['status'] {
	if (notice !== null) return { message: format(JUMP_FAILURE_MESSAGES[notice]), warning: true };
	if (jumping) return { message: format(workbenchMessages.agentBoardJumping), warning: false };
	if (rows.length === 0) return null;
	switch (read.kind) {
		case 'failed':
			return { message: format(READ_FAILURE_MESSAGES[read.reason]), warning: true };
		case 'listed':
			return read.complete
				? null
				: { message: format(workbenchMessages.agentBoardReadIncomplete), warning: false };
		case 'reading':
			return { message: format(workbenchMessages.agentBoardReadReading), warning: false };
		case 'unread':
			return null;
	}
}
