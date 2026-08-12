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
						{intl.formatMessage(projectedTerminalMessages.pasteReviewTitle)}
					</AlertDialogTitle>
					<AlertDialogDescription>
						{intl.formatMessage(projectedTerminalMessages.pasteReviewDescription)}
					</AlertDialogDescription>
				</AlertDialogHeader>
				<div className={styles['review']}>
					<ul className={styles['warnings']}>
						{review.multiline ? (
							<li>{intl.formatMessage(projectedTerminalMessages.pasteReviewMultiline)}</li>
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
						aria-label={intl.formatMessage(projectedTerminalMessages.pasteReviewOriginal)}
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
						{intl.formatMessage(projectedTerminalMessages.pasteReviewConfirm)}
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}
