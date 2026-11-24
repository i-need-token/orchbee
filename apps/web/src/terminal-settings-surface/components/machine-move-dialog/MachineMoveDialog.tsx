import {
	Button,
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	Kbd,
} from '@orchbee/ui/components';
import * as React from 'react';

import { useArmedChoice } from '../../hooks/useArmedChoice';
import { useTerminalSettingsMachines } from '../../hooks/useTerminalSettingsMachines';
import type {
	TerminalSettingsMachineMoveQuestion,
	TerminalSettingsMachines,
} from '../../types/terminal-settings-surface';
import styles from './MachineMoveDialog.module.css';

interface MachineMoveDialogProps {
	readonly machines: TerminalSettingsMachines;
}

/**
 * Change machine's question, as the TUI asks it before a move stops a program: the Workspace, what
 * would stop or that what runs could not be read, and that files are not moved, with Move in the
 * destructive tone. It appears on its own once the check answers, so Enter and Move act only once
 * it has shown for a second with no key pressed on it; Esc and Cancel give the move up.
 */
export function MachineMoveDialog({ machines }: MachineMoveDialogProps): React.ReactNode {
	const question = useTerminalSettingsMachines(machines)?.move.question ?? null;
	if (question === null) return null;
	return <MachineMoveQuestion key={question.question} machines={machines} question={question} />;
}

function MachineMoveQuestion({
	machines,
	question,
}: {
	readonly machines: TerminalSettingsMachines;
	readonly question: TerminalSettingsMachineMoveQuestion;
}): React.ReactNode {
	const armed = useArmedChoice('open');
	const primary = React.useRef<HTMLButtonElement>(null);
	const answer = (confirm: boolean): void => {
		if (confirm && !armed.now) {
			armed.restart();
			return;
		}
		machines.request({ act: 'move-answer', confirm, question: question.question });
	};
	return (
		<Dialog
			disablePointerDismissal
			open
			onOpenChange={(open) => {
				if (!open) answer(false);
			}}
		>
			<DialogContent
				data-slot='machine-move-dialog'
				initialFocus={primary}
				showCloseButton={false}
				onKeyDownCapture={(event) => {
					// Every key but an armed Enter or Escape starts the second again; Enter on a button
					// presses that button.
					if (event.key === 'Enter') {
						if (event.target instanceof HTMLButtonElement) return;
						event.preventDefault();
						answer(true);
					} else if (event.key !== 'Escape') {
						armed.restart();
					}
				}}
			>
				<DialogHeader className={styles['header']}>
					<DialogTitle className={styles['title']}>{question.title}</DialogTitle>
				</DialogHeader>
				<div className={styles['body']}>
					<p className={styles['workspace']}>{question.workspace}</p>
					<p className={styles['line']}>{question.sentence}</p>
					<p className={styles['line']}>{question.files}</p>
				</div>
				<DialogFooter>
					<Button
						className={styles['action']}
						type='button'
						variant='outline'
						onClick={() => answer(false)}
					>
						{question.cancel}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Esc
						</Kbd>
					</Button>
					<Button
						ref={primary}
						className={styles['action']}
						data-slot='machine-move-confirm'
						type='button'
						variant='destructive'
						onClick={() => answer(true)}
					>
						{question.move}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Enter
						</Kbd>
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
