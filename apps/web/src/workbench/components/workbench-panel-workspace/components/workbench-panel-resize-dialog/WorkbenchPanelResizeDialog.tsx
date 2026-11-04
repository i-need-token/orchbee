import {
	Button,
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	Kbd,
} from '@orchbee/ui/components';
import * as React from 'react';

import { PANEL_RESIZE_OPTIONS } from '../../constants/panel-resize-options';
import { useWorkbenchPanelResizeDialog } from '../../hooks/useWorkbenchPanelResizeDialog';
import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import styles from './WorkbenchPanelResizeDialog.module.css';

interface WorkbenchPanelResizeDialogProps {
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanelResizeDialog({
	view,
}: WorkbenchPanelResizeDialogProps): React.ReactNode {
	const dialog = useWorkbenchPanelResizeDialog(view);
	// Not `autoFocus`: the menu this can open from returns focus as it closes, after that has run.
	const firstAvailableOption = React.useRef<HTMLButtonElement>(null);
	const pendingResize = view.pendingResize;
	if (pendingResize === null) return null;
	const firstAvailableIndex = PANEL_RESIZE_OPTIONS.findIndex((option) =>
		view.canResizePanel(pendingResize.panelId, option.adjustment),
	);

	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) view.cancelPanelResize();
			}}
			open
		>
			<DialogContent
				closeLabel={view.messages.close}
				data-slot='workbench-panel-resize-dialog'
				finalFocus={dialog.finalFocus}
				initialFocus={firstAvailableOption}
				onKeyDownCapture={dialog.onKeyDownCapture}
			>
				<DialogHeader className={styles['header']}>
					<DialogTitle className={styles['title']}>{view.messages.resizeTitle}</DialogTitle>
					<DialogDescription className={styles['description']}>
						{view.messages.resizeDescription}
					</DialogDescription>
				</DialogHeader>
				<div className={styles['options']}>
					{PANEL_RESIZE_OPTIONS.map((option, index) => (
						<Button
							className={styles['option']}
							disabled={!view.canResizePanel(pendingResize.panelId, option.adjustment)}
							// The key that just reached the limit must leave the keyboard in this dialog.
							focusableWhenDisabled
							key={option.adjustment}
							ref={index === firstAvailableIndex ? firstAvailableOption : undefined}
							type='button'
							variant='outline'
							onClick={() => dialog.resize(option.adjustment, 1)}
						>
							<span>{view.messages[option.message]}</span>
							<Kbd aria-hidden='true'>{option.key}</Kbd>
						</Button>
					))}
				</div>
				<p aria-live='polite' className={styles['status']} data-slot='panel-resize-status'>
					{dialog.status ?? '\u00a0'}
				</p>
				<DialogFooter>
					<Button className={styles['action']} type='button' onClick={view.cancelPanelResize}>
						{view.messages.resizeDone}
						<Kbd aria-hidden='true' className={styles['doneShortcut']}>
							Esc
						</Kbd>
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
