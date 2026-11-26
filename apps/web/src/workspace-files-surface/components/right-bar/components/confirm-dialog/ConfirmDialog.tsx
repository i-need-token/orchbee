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

import styles from './ConfirmDialog.module.css';

/** At most this many files are named; the rest are counted. */
const LISTED_FILES = 8;

/** A question asked before a change to the Workspace's files or repository. */
export interface ConfirmQuestion {
	readonly title: string;
	readonly description: string;
	readonly confirm: string;
	/** Files the change touches, listed when there is more than one. */
	readonly paths: readonly string[];
	/**
	 * A change that cannot be undone keeps the focus on Cancel, so Enter never makes it by accident;
	 * any other keeps it on the confirming action.
	 */
	readonly destructive: boolean;
}

interface ConfirmDialogProps {
	/** The question; null while nothing asks. */
	readonly question: ConfirmQuestion | null;
	readonly onConfirm: () => void;
	readonly onCancel: () => void;
}

/**
 * Asks before a change the right bar makes: discarding changes, committing everything when nothing
 * is staged, or replacing every match a search found.
 */
export function ConfirmDialog({
	question,
	onConfirm,
	onCancel,
}: ConfirmDialogProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const cancelButton = React.useRef<HTMLButtonElement>(null);
	const confirmButton = React.useRef<HTMLButtonElement>(null);
	// The question keeps its words while the dialog fades out after the person answered it.
	const [asked, setAsked] = React.useState(question);
	if (question !== null && question !== asked) setAsked(question);
	if (asked === null) return null;
	const listed = asked.paths.length > 1 ? asked.paths.slice(0, LISTED_FILES) : [];
	const more = asked.paths.length - listed.length;
	return (
		<AlertDialog
			onOpenChange={(open) => {
				if (!open) onCancel();
			}}
			open={question !== null}
		>
			<AlertDialogContent
				className={styles['content']}
				initialFocus={asked.destructive ? cancelButton : confirmButton}
			>
				<AlertDialogHeader>
					<AlertDialogTitle className={styles['title']}>{asked.title}</AlertDialogTitle>
					<AlertDialogDescription className={styles['description']}>
						{asked.description}
					</AlertDialogDescription>
					{listed.length > 0 ? (
						<ul className={styles['files']}>
							{listed.map((path) => (
								<li key={path}>{path}</li>
							))}
							{more > 0 ? (
								<li className={styles['more']}>
									{intl.formatMessage(workbenchMessages.rightBarConfirmMore, { count: more })}
								</li>
							) : null}
						</ul>
					) : null}
				</AlertDialogHeader>
				<AlertDialogFooter className={styles['footer']}>
					<AlertDialogCancel ref={cancelButton}>
						{intl.formatMessage(workbenchMessages.rightBarConfirmCancel)}
					</AlertDialogCancel>
					<AlertDialogAction
						onClick={onConfirm}
						ref={confirmButton}
						variant={asked.destructive ? 'destructive' : 'default'}
					>
						{asked.confirm}
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}
