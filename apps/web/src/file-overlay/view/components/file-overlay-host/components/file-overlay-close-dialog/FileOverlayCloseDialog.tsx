import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileOverlayMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
	Kbd,
} from '@orchbee/ui/components';
import { useRef, useState, type ReactNode } from 'react';

import { splitFileTabPath, type FileOverlayState } from '../../../../../model';
import {
	useFileOverlayHostDispatch,
	useFileOverlayHostFocusRequest,
	useFileOverlayHostReadState,
	useFileOverlayHostReturnFocus,
	useFileOverlayHostStore,
} from '../../contexts/file-overlay-host-store';
import styles from './FileOverlayCloseDialog.module.css';

/** How many files the question about several tabs names, as VS Code's names at most. */
const NAMED_FILES = 10;

type PendingClose = NonNullable<FileOverlayState['tabs']['pendingClose']>;

/** The question one waiting close asks: about its unsaved tabs, each by its file's name. */
interface FileOverlaySaveQuestion {
	readonly files: readonly { readonly id: string; readonly name: string }[];
	readonly pendingClose: PendingClose;
}

function askedAbout(state: FileOverlayState, pendingClose: PendingClose): FileOverlaySaveQuestion {
	const files = state.tabs.tabs
		.filter((tab) => tab.unsaved && pendingClose.ids.includes(tab.id))
		.map((tab) => ({ id: tab.id, name: splitFileTabPath(tab.input.resource.path).name }));
	return { files, pendingClose };
}

/**
 * VS Code's save question for a close that takes tabs with unsaved edits: one such tab asks about its
 * file, several list theirs. Save, or Save All, holds the focus so Enter saves, and Esc cancels,
 * keeping every tab. As the question closes, the active tab takes the keys while the overlay still
 * shows; once the overlay has closed they go back where they came from.
 */
export function FileOverlayCloseDialog(): ReactNode {
	const intl = useOrchbeeIntl();
	const dispatch = useFileOverlayHostDispatch();
	const focusRequest = useFileOverlayHostFocusRequest();
	const readState = useFileOverlayHostReadState();
	const returnFocus = useFileOverlayHostReturnFocus();
	const pendingClose = useFileOverlayHostStore((state) => state.tabs.pendingClose);
	const deciding = pendingClose?.phase === 'deciding';
	// The question keeps its words while it fades out after the person answered it.
	const [question, setQuestion] = useState<FileOverlaySaveQuestion | null>(null);
	if (deciding && pendingClose !== question?.pendingClose) {
		setQuestion(askedAbout(readState(), pendingClose));
	}
	const saveButton = useRef<HTMLButtonElement>(null);
	const decide = (decision: 'cancel' | 'discard' | 'save'): void => {
		dispatch({ decision, type: 'close-decided' });
	};
	const giveFocus = useMemoizedFn((): false => {
		if (readState().mode.kind === 'closed') returnFocus();
		else focusRequest.ask();
		return false;
	});
	const files = question?.files ?? [];
	const unnamed = files.length - NAMED_FILES;
	return (
		<AlertDialog
			onOpenChange={(open) => {
				if (!open) decide('cancel');
			}}
			open={deciding}
		>
			<AlertDialogContent
				className={styles['content']}
				finalFocus={giveFocus}
				initialFocus={saveButton}
			>
				<AlertDialogHeader>
					<AlertDialogTitle className={styles['title']}>
						{intl.formatMessage(fileOverlayMessages.saveQuestion, {
							count: files.length,
							name: files[0]?.name ?? '',
						})}
					</AlertDialogTitle>
					<AlertDialogDescription className={styles['description']}>
						{files.length > 1 ? (
							<span className={styles['files']}>
								{files.slice(0, NAMED_FILES).map((file) => (
									<span className={styles['file']} key={file.id}>
										{file.name}
									</span>
								))}
								{unnamed > 0 ? (
									<span className={styles['file']}>
										{intl.formatMessage(fileOverlayMessages.saveQuestionMoreFiles, {
											count: unnamed,
										})}
									</span>
								) : null}
							</span>
						) : null}
						<span className={styles['consequence']}>
							{intl.formatMessage(fileOverlayMessages.saveQuestionDetail)}
						</span>
					</AlertDialogDescription>
				</AlertDialogHeader>
				<AlertDialogFooter className={styles['footer']}>
					<AlertDialogAction onClick={() => decide('discard')} variant='outline'>
						{intl.formatMessage(fileOverlayMessages.saveQuestionDontSave)}
					</AlertDialogAction>
					<AlertDialogCancel>
						{intl.formatMessage(fileOverlayMessages.saveQuestionCancel)}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Esc
						</Kbd>
					</AlertDialogCancel>
					{/* A close confirmation focuses its primary action, so Enter saves. */}
					<AlertDialogAction onClick={() => decide('save')} ref={saveButton}>
						{intl.formatMessage(
							files.length > 1
								? fileOverlayMessages.saveQuestionSaveAll
								: fileOverlayMessages.saveQuestionSave,
						)}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Enter
						</Kbd>
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}
