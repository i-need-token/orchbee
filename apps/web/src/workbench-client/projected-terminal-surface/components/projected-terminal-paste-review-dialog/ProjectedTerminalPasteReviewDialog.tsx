import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ProjectedPasteReview } from '@orchbee/terminal';
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
import type { ReactNode } from 'react';

import styles from './ProjectedTerminalPasteReviewDialog.module.css';
import { projectedTerminalPastePreview } from './utils/projectedTerminalPastePreview';

interface ProjectedTerminalPasteReviewDialogProps {
	readonly review: ProjectedPasteReview;
	onApprove(): void;
	onCancel(): void;
}

export function ProjectedTerminalPasteReviewDialog({
	review,
	onApprove,
	onCancel,
}: ProjectedTerminalPasteReviewDialogProps): ReactNode {
	const intl = useOrchbeeIntl();
	const preview = projectedTerminalPastePreview(review.originalText);
	const fileDrop = review.source === 'file-drop';
	return (
		<AlertDialog
			onOpenChange={(open) => {
				if (!open) onCancel();
			}}
			open
		>
			<AlertDialogContent data-slot='projected-terminal-paste-review'>
				<AlertDialogHeader>
					<AlertDialogTitle>
						{intl.formatMessage(
							fileDrop
								? projectedTerminalMessages.fileDropReviewTitle
								: projectedTerminalMessages.pasteReviewTitle,
						)}
					</AlertDialogTitle>
					<AlertDialogDescription>
						{intl.formatMessage(
							fileDrop
								? projectedTerminalMessages.fileDropReviewDescription
								: projectedTerminalMessages.pasteReviewDescription,
						)}
					</AlertDialogDescription>
				</AlertDialogHeader>
				<div className={styles['review']}>
					<ul className={styles['warnings']}>
						{fileDrop ? (
							<li>{intl.formatMessage(projectedTerminalMessages.fileDropReviewRemote)}</li>
						) : null}
						{review.multiline ? (
							<li>
								{intl.formatMessage(
									fileDrop
										? projectedTerminalMessages.fileDropReviewNewline
										: projectedTerminalMessages.pasteReviewMultiline,
								)}
							</li>
						) : null}
						{review.replacedControlCharacters > 0 ? (
							<li>
								{intl.formatMessage(projectedTerminalMessages.pasteReviewControls, {
									count: review.replacedControlCharacters,
								})}
							</li>
						) : null}
						{review.embeddedBracketTerminator ? (
							<li>{intl.formatMessage(projectedTerminalMessages.pasteReviewTerminator)}</li>
						) : null}
					</ul>
					<pre
						aria-label={intl.formatMessage(
							fileDrop
								? projectedTerminalMessages.fileDropReviewInput
								: projectedTerminalMessages.pasteReviewOriginal,
						)}
						className={styles['preview']}
					>
						{preview.text}
					</pre>
					{preview.truncated ? (
						<p className={styles['truncated']}>
							{intl.formatMessage(projectedTerminalMessages.pasteReviewTruncated)}
						</p>
					) : null}
				</div>
				<AlertDialogFooter>
					<AlertDialogCancel>
						{intl.formatMessage(projectedTerminalMessages.pasteReviewCancel)}
					</AlertDialogCancel>
					<AlertDialogAction onClick={onApprove}>
						{intl.formatMessage(
							fileDrop
								? projectedTerminalMessages.fileDropReviewConfirm
								: projectedTerminalMessages.pasteReviewConfirm,
						)}
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}
