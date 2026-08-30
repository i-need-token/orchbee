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

export function WorkbenchPanelCloseDialog({
	view,
}: WorkbenchPanelCloseDialogProps): React.ReactNode {
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
						{view.messages.closeRunningTitle}
					</AlertDialogTitle>
					<AlertDialogDescription className={styles['description']}>
						{view.messages.closeRunningDescription(view.pendingClose?.runningCount ?? 0)}
					</AlertDialogDescription>
				</AlertDialogHeader>
				<AlertDialogFooter className={styles['footer']}>
					<AlertDialogCancel>{view.messages.cancel}</AlertDialogCancel>
					<AlertDialogAction onClick={view.confirmClose} variant='destructive'>
						{view.messages.closeAnyway}
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}
