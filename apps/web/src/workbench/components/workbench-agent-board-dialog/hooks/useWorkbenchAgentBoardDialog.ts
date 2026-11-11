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
	/** Present while an Agent can be started: the first row and `S` open Start Agent. */
	readonly onStartAgent: (() => void) | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
}

interface WorkbenchAgentBoardDialogRowControl {
	readonly elementId: string;
	readonly elementRef: React.RefObject<HTMLDivElement | null> | undefined;
	readonly onClick: () => void;
	readonly onDoubleClick: () => void;
	readonly selected: boolean;
}

interface WorkbenchAgentBoardDialogRow
	extends WorkbenchAgentBoardRowView, WorkbenchAgentBoardDialogRowControl {}

interface WorkbenchAgentBoardDialogStartRow extends WorkbenchAgentBoardDialogRowControl {
	readonly label: string;
}

/** The row the selection is on: the first row, which opens Start Agent, or a session. */
type WorkbenchAgentBoardChoice =
	| { readonly kind: 'session'; readonly sessionId: string }
	| { readonly kind: 'start' };

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
	/** The always-present first row while an Agent can be started. */
	readonly startRow: WorkbenchAgentBoardDialogStartRow | null;
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
 * The open board as the TUI keeps it: an episode for as long as it is mounted, the Start Agent row
 * above the sessions in the port's order, a selection that follows its row and starts on the first
 * session, or on the Start Agent row when there is none, and one status line in which a stopped jump
 * outranks a jump under way, which outranks the state of the last read.
 */
export function useWorkbenchAgentBoardDialog({
	board,
	onShown,
	onStartAgent,
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
	const [chosen, setChosen] = React.useState<WorkbenchAgentBoardChoice | null>(null);

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
	// The Start Agent row, when present, is index 0 and the sessions follow it.
	const offset = onStartAgent === undefined ? 0 : 1;
	const chosenIndex = rowIndex(chosen, offset, rowViews);
	// Without a choice still on the board, the first session, or the Start Agent row without one.
	const selectedIndex = chosenIndex === -1 ? Math.min(offset, rowViews.length) : chosenIndex;
	const startSelected = offset === 1 && selectedIndex === 0;
	const selectedSessionId = startSelected
		? null
		: (rowViews[selectedIndex - offset]?.sessionId ?? null);

	React.useEffect(() => {
		selectedElement.current?.scrollIntoView?.({ block: 'nearest' });
	}, [selectedSessionId, startSelected]);

	const jump = useMemoizedFn((sessionId: string) => {
		setChosen({ kind: 'session', sessionId });
		void board.jump(sessionId).then((outcome) => {
			if (outcome === 'shown') onShown();
		});
	});
	const startAgent = useMemoizedFn(() => {
		setChosen({ kind: 'start' });
		onStartAgent?.();
	});
	const open = useMemoizedFn(() => {
		if (startSelected) startAgent();
		else if (selectedSessionId !== null) jump(selectedSessionId);
	});
	const refresh = useMemoizedFn(() => board.refresh());
	const select = (index: number): void => {
		const clamped = Math.min(Math.max(index, 0), offset + rowViews.length - 1);
		if (offset === 1 && clamped === 0) {
			setChosen({ kind: 'start' });
			return;
		}
		const row = rowViews[clamped - offset];
		if (row !== undefined) setChosen({ kind: 'session', sessionId: row.sessionId });
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
				select(offset + rowViews.length - 1);
				break;
			case 'r':
				refresh();
				break;
			case 's':
			case 'S':
				if (onStartAgent === undefined) return;
				startAgent();
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

	const startElementId = `${elementIdPrefix}-start`;
	const sessionElementId = (sessionId: string): string => `${elementIdPrefix}-session-${sessionId}`;
	let activeDescendant: string | undefined;
	if (startSelected) activeDescendant = startElementId;
	else if (selectedSessionId !== null) activeDescendant = sessionElementId(selectedSessionId);

	return {
		activeDescendant,
		canOpen: startSelected || selectedSessionId !== null,
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
				elementId: sessionElementId(row.sessionId),
				elementRef: selected ? selectedElement : undefined,
				onClick: () => setChosen({ kind: 'session', sessionId: row.sessionId }),
				onDoubleClick: () => jump(row.sessionId),
				selected,
			};
		}),
		selectLabel: intl.formatMessage(workbenchMessages.agentBoardSelect),
		startRow:
			onStartAgent === undefined
				? null
				: {
						elementId: startElementId,
						elementRef: startSelected ? selectedElement : undefined,
						label: intl.formatMessage(workbenchMessages.agentBoardStartRow),
						onClick: () => setChosen({ kind: 'start' }),
						onDoubleClick: startAgent,
						selected: startSelected,
					},
		status: projectStatus(snapshot, (descriptor) => intl.formatMessage(descriptor)),
		title: intl.formatMessage(workbenchMessages.agentBoardTitle),
	};
}

/**
 * Where `chosen` sits among the board's rows, the sessions following `offset` rows before them, or
 * -1 when the board no longer shows it.
 */
function rowIndex(
	chosen: WorkbenchAgentBoardChoice | null,
	offset: number,
	rows: readonly WorkbenchAgentBoardRowView[],
): number {
	if (chosen === null) return -1;
	if (chosen.kind === 'start') return offset - 1;
	const sessionIndex = rows.findIndex((row) => row.sessionId === chosen.sessionId);
	return sessionIndex === -1 ? -1 : sessionIndex + offset;
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
