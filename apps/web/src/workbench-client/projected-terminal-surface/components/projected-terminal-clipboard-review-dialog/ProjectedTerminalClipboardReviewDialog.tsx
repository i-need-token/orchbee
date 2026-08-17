import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { TerminalClipboardWriteRequest } from '@orchbee/terminal-protocol';
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

import styles from './ProjectedTerminalClipboardReviewDialog.module.css';

interface ProjectedTerminalClipboardReviewDialogProps {
	readonly request: TerminalClipboardWriteRequest;
	onApprove(): void;
	onCancel(): void;
}

export function ProjectedTerminalClipboardReviewDialog({
	request,
	onApprove,
	onCancel,
}: ProjectedTerminalClipboardReviewDialogProps): ReactNode {
	const intl = useOrchbeeIntl();
	const byteLength = new TextEncoder().encode(request.text).byteLength;
	return (
		<AlertDialog
			onOpenChange={(open) => {
				if (!open) onCancel();
			}}
			open
		>
			<AlertDialogContent data-slot='projected-terminal-clipboard-review'>
				<AlertDialogHeader>
					<AlertDialogTitle>
						{intl.formatMessage(projectedTerminalMessages.clipboardReviewTitle)}
					</AlertDialogTitle>
					<AlertDialogDescription>
						{intl.formatMessage(projectedTerminalMessages.clipboardReviewDescription)}
					</AlertDialogDescription>
				</AlertDialogHeader>
				<div className={styles['review']}>
					<pre
						aria-label={intl.formatMessage(projectedTerminalMessages.clipboardReviewContent)}
						className={styles['preview']}
					>
						{request.text}
					</pre>
					<p className={styles['bytes']}>
						{intl.formatMessage(projectedTerminalMessages.clipboardReviewBytes, { byteLength })}
					</p>
				</div>
				<AlertDialogFooter>
					<AlertDialogCancel>
						{intl.formatMessage(projectedTerminalMessages.clipboardReviewCancel)}
					</AlertDialogCancel>
					<AlertDialogAction onClick={onApprove}>
						{intl.formatMessage(projectedTerminalMessages.clipboardReviewConfirm)}
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}
