import { codeEditorMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactNode } from 'react';

import type { FileError } from '../../../file-session';
import { CodeEditorPlaceholder } from '../code-editor-placeholder/CodeEditorPlaceholder';

interface CodeEditorErrorPlaceholderProps {
	readonly error: FileError;
	readonly onRetry: () => void;
}

/**
 * VS Code's placeholder for a file that failed to open: a missing file or a directory says so, and
 * any other failure offers Try Again.
 */
export function CodeEditorErrorPlaceholder({
	error,
	onRetry,
}: CodeEditorErrorPlaceholderProps): ReactNode {
	const intl = useOrchbeeIntl();
	if (error.code === 'not-found') {
		return (
			<CodeEditorPlaceholder
				actions={[]}
				message={intl.formatMessage(codeEditorMessages.notFound)}
			/>
		);
	}
	if (error.code === 'is-directory') {
		return (
			<CodeEditorPlaceholder
				actions={[]}
				message={intl.formatMessage(codeEditorMessages.directory)}
			/>
		);
	}
	return (
		<CodeEditorPlaceholder
			actions={[{ label: intl.formatMessage(codeEditorMessages.retry), run: onRetry }]}
			message={intl.formatMessage(codeEditorMessages.failed)}
		/>
	);
}
