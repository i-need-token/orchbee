import {
	Button,
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	Kbd,
} from '@orchbee/ui/components';
import type * as React from 'react';

import type { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAgentBoard } from '../../types/workbench-agent-board';
import { WorkbenchAgentStateGlyph } from '../workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import { useWorkbenchAgentBoardDialog } from './hooks/useWorkbenchAgentBoardDialog';
import styles from './WorkbenchAgentBoardDialog.module.css';

interface WorkbenchAgentBoardDialogProps {
	readonly board: WorkbenchAgentBoard;
	readonly finalFocus: () => HTMLElement | false | null;
	readonly onClose: () => void;
	readonly onShown: () => void;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
}

/** The Server's Agent sessions, one per row, from which a jump shows a session's terminal. */
export function WorkbenchAgentBoardDialog({
	board,
	finalFocus,
	onClose,
	onShown,
	projectWorkspaceRuntime,
}: WorkbenchAgentBoardDialogProps): React.ReactNode {
	const view = useWorkbenchAgentBoardDialog({ board, onShown, projectWorkspaceRuntime });

	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
			open
		>
			<DialogContent
				className={styles['surface']}
				closeLabel={view.closeLabel}
				data-slot='workbench-agent-board'
				finalFocus={finalFocus}
				initialFocus={view.listRef}
				onKeyDown={view.onKeyDown}
			>
				<DialogHeader className={styles['header']}>
					<DialogTitle className={styles['title']}>{view.title}</DialogTitle>
				</DialogHeader>
				<div
					aria-activedescendant={view.activeDescendant}
					aria-label={view.listLabel}
					className={styles['list']}
					data-slot='workbench-agent-board-list'
					ref={view.listRef}
					role='listbox'
					tabIndex={0}
				>
					{view.rows.length === 0 ? (
						<p className={styles['empty']} data-slot='workbench-agent-board-empty'>
							{view.emptyMessage}
						</p>
					) : (
						view.rows.map((row) => (
							<div
								aria-selected={row.selected}
								className={styles['row']}
								data-ended={row.ended}
								data-highlighted={row.selected}
								id={row.elementId}
								key={row.sessionId}
								ref={row.elementRef}
								role='option'
								onClick={row.onClick}
								onDoubleClick={row.onDoubleClick}
							>
								<WorkbenchAgentStateGlyph status={row.glyph} />
								<span className={styles['status']}>{row.status}</span>
								<span className={styles['detail']}>{row.detail}</span>
							</div>
						))
					)}
				</div>
				<p
					aria-live='polite'
					className={styles['statusLine']}
					data-slot='workbench-agent-board-status'
					data-warning={view.status?.warning === true}
				>
					{view.status?.message ?? ' '}
				</p>
				<DialogFooter className={styles['footer']}>
					<span className={styles['hint']}>
						<span className={styles['keyGroup']}>
							<Kbd>↑</Kbd>
							<Kbd>↓</Kbd>
						</span>
						{view.selectLabel}
					</span>
					<Button
						className={styles['action']}
						type='button'
						variant='outline'
						onClick={view.refresh}
					>
						{view.refreshLabel}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							R
						</Kbd>
					</Button>
					<Button
						className={styles['action']}
						disabled={!view.canOpen}
						type='button'
						onClick={view.open}
					>
						{view.openLabel}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							↵
						</Kbd>
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
