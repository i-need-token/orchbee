import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import styles from './WorkbenchPanelCloseDialog.module.css';

interface WorkbenchPanelCloseDialogProps {
	readonly view: WorkbenchPanelWorkspaceView;
}

/** Names what closes and where, then what closing ends, in the TUI's order. */
export function WorkbenchPanelCloseDialog({
	view,
}: WorkbenchPanelCloseDialogProps): React.ReactNode {
	// The question keeps its words while the dialog fades out after the person answered it.
	const [close, setClose] = React.useState(view.pendingClose);
	if (view.pendingClose !== null && view.pendingClose !== close) {
		setClose(view.pendingClose);
	}
	return (
		<AlertDialog
			onOpenChange={(open) => {
				if (!open) {
					view.cancelClose();
				}
			}}
			open={view.pendingClose !== null}
		>
			<AlertDialogContent className={styles['content']}>
				<AlertDialogHeader className={styles['header']}>
					<AlertDialogTitle className={styles['title']}>
						{close?.childTerminal === true
							? view.messages.closeChildTitle
							: view.messages.closeTitle}
					</AlertDialogTitle>
					{close === null ? null : (
						<AlertDialogDescription className={styles['description']}>
							<span className={styles['target']}>{close.target}</span>
							<span className={styles['line']}>{view.messages.closeTab(close.tabTitle)}</span>
							<span className={styles['consequence']}>
								{view.messages.closeWarning(close.impact)}
							</span>
						</AlertDialogDescription>
					)}
				</AlertDialogHeader>
				<AlertDialogFooter className={styles['footer']}>
					<AlertDialogCancel>{view.messages.cancel}</AlertDialogCancel>
					<AlertDialogAction onClick={view.confirmClose} variant='destructive'>
						{view.messages.close}
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}
