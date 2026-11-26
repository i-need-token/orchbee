import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
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

import styles from './UnsavedCloseDialog.module.css';

interface UnsavedCloseDialogProps {
	/** The Workspace-relative files whose edits closing would lose; null while nothing asks. */
	readonly paths: readonly string[] | null;
	readonly onSave: () => void;
	readonly onDiscard: () => void;
	readonly onCancel: () => void;
}

function nameOf(path: string): string {
	return path.slice(path.lastIndexOf('/') + 1);
}

/**
 * Asks before closing files whose edits are not saved, as an editor does: save them, close without
 * them, or keep the files open. Save holds the focus, so Enter saves; Escape keeps them open.
 */
export function UnsavedCloseDialog({
	paths,
	onSave,
	onDiscard,
	onCancel,
}: UnsavedCloseDialogProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const saveButton = React.useRef<HTMLButtonElement>(null);
	// The question keeps its words while the dialog fades out after the person answered it.
	const [asked, setAsked] = React.useState(paths);
	if (paths !== null && paths !== asked) setAsked(paths);
	const shown = asked ?? [];
	const [first] = shown;
	return (
		<AlertDialog
			onOpenChange={(open) => {
				if (!open) onCancel();
			}}
			open={paths !== null}
		>
			<AlertDialogContent className={styles['content']} initialFocus={saveButton}>
				<AlertDialogHeader>
					<AlertDialogTitle className={styles['title']}>
						{shown.length === 1 && first !== undefined
							? intl.formatMessage(workbenchMessages.rightBarCloseTitleOne, { name: nameOf(first) })
							: intl.formatMessage(workbenchMessages.rightBarCloseTitleMany, {
									count: shown.length,
								})}
					</AlertDialogTitle>
					<AlertDialogDescription className={styles['description']}>
						{intl.formatMessage(workbenchMessages.rightBarCloseDescription)}
					</AlertDialogDescription>
					{shown.length > 1 ? (
						<ul className={styles['files']}>
							{shown.map((path) => (
								<li key={path}>{path}</li>
							))}
						</ul>
					) : null}
				</AlertDialogHeader>
				<AlertDialogFooter className={styles['footer']}>
					<AlertDialogCancel>
						{intl.formatMessage(workbenchMessages.rightBarCloseCancel)}
					</AlertDialogCancel>
					<AlertDialogAction onClick={onDiscard} variant='outline'>
						{intl.formatMessage(workbenchMessages.rightBarCloseDiscard)}
					</AlertDialogAction>
					<AlertDialogAction onClick={onSave} ref={saveButton}>
						{intl.formatMessage(workbenchMessages.rightBarFloatSave)}
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}
