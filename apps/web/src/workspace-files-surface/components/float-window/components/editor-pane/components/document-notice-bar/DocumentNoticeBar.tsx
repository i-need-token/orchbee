import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import { Button, ProductIcon } from '@orchbee/ui/components';
import type * as React from 'react';

import type { DocumentNotice } from '../../../../../../EditorDocuments';

import styles from './DocumentNoticeBar.module.css';

/** What the person can do about a notice; each is offered only where it applies. */
interface DocumentNoticeActions {
	/** Shows the file on disk beside the edits; absent where there is no file to compare. */
	readonly onCompare?: (() => void) | undefined;
	readonly onStopComparing?: (() => void) | undefined;
	/** Saves over the file on disk; absent where a save cannot check what it replaces. */
	readonly onOverwrite?: (() => void) | undefined;
	readonly onDiscard: () => void;
	readonly onDismiss: () => void;
}

interface DocumentNoticeBarProps extends DocumentNoticeActions {
	readonly notice: DocumentNotice;
	/** The size a save carries, named by the notices about it, such as 10 MB. */
	readonly limit: string;
	/** True while a save runs, which no action may start beside. */
	readonly busy: boolean;
}

/**
 * Why an edited file's text is not on disk, with what can be done: a conflict offers comparing,
 * overwriting and discarding the edits; a failed save can only be dismissed, and saving again
 * tries again.
 */
export function DocumentNoticeBar({
	notice,
	limit,
	busy,
	onCompare,
	onStopComparing,
	onOverwrite,
	onDiscard,
	onDismiss,
}: DocumentNoticeBarProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const action = (label: string, onClick: () => void): React.ReactNode => (
		<Button disabled={busy} onClick={onClick} size='xs' variant='outline'>
			{label}
		</Button>
	);
	let text: string;
	if (notice.kind === 'conflict') {
		if (notice.version === null) text = intl.formatMessage(workbenchMessages.rightBarNoticeDeleted);
		else if (notice.version.sha256 === null) {
			text = intl.formatMessage(workbenchMessages.rightBarNoticeConflictTooLarge, { limit });
		} else text = intl.formatMessage(workbenchMessages.rightBarNoticeConflict);
	} else if (notice.kind === 'too-large') {
		text = intl.formatMessage(workbenchMessages.rightBarNoticeTooLarge, { limit });
	} else {
		text = intl.formatMessage(
			{
				denied: workbenchMessages.rightBarNoticeDenied,
				'not-found': workbenchMessages.rightBarNoticeNotFound,
				unavailable: workbenchMessages.rightBarNoticeUnavailable,
				unconfirmed: workbenchMessages.rightBarNoticeUnconfirmed,
			}[notice.reason],
		);
	}
	const dismissLabel = intl.formatMessage(workbenchMessages.rightBarNoticeDismiss);
	return (
		<div
			className={styles['root']}
			data-slot='workspace-document-notice'
			data-tone={notice.kind === 'conflict' ? 'warning' : 'danger'}
			role='alert'
		>
			<span className={styles['icon']}>
				<ProductIcon name='warning' />
			</span>
			<p className={styles['text']}>{text}</p>
			<div className={styles['actions']}>
				{notice.kind === 'conflict' ? (
					<>
						{onStopComparing === undefined
							? null
							: action(
									intl.formatMessage(workbenchMessages.rightBarNoticeStopComparing),
									onStopComparing,
								)}
						{onCompare === undefined
							? null
							: action(intl.formatMessage(workbenchMessages.rightBarNoticeCompare), onCompare)}
						{onOverwrite === undefined
							? null
							: action(
									intl.formatMessage(
										notice.version === null
											? workbenchMessages.rightBarFloatSave
											: workbenchMessages.rightBarNoticeOverwrite,
									),
									onOverwrite,
								)}
						{action(intl.formatMessage(workbenchMessages.rightBarNoticeDiscard), onDiscard)}
					</>
				) : (
					<Button
						aria-label={dismissLabel}
						onClick={onDismiss}
						size='icon-xs'
						title={dismissLabel}
						variant='ghost'
					>
						<ProductIcon name='close' />
					</Button>
				)}
			</div>
		</div>
	);
}
