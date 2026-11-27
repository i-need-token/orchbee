import { codeEditorMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactNode } from 'react';

import type { CodeEditorTextDocumentState } from '../../CodeEditorTextDocument';
import { CodeEditorErrorPlaceholder } from '../code-editor-error-placeholder/CodeEditorErrorPlaceholder';
import { CodeEditorPlaceholder } from '../code-editor-placeholder/CodeEditorPlaceholder';

interface CodeEditorDocumentPlaceholderProps {
	/** The document's own recoveries, which the placeholder's buttons run. */
	readonly document: { openAnyway(): void; retry(): void };
	/** The file's name, which a refusal quotes. */
	readonly name: string;
	readonly state: CodeEditorTextDocumentState<unknown>;
}

/**
 * VS Code's placeholder for a document the text editor refuses or failed to read, or nothing while
 * it loads and once it shows.
 */
export function CodeEditorDocumentPlaceholder({
	document,
	name,
	state,
}: CodeEditorDocumentPlaceholderProps): ReactNode {
	const intl = useOrchbeeIntl();
	const openAnyway = {
		label: intl.formatMessage(codeEditorMessages.openAnyway),
		run: () => document.openAnyway(),
	};
	switch (state.kind) {
		case 'loading':
		case 'text':
			return null;
		case 'binary':
			return (
				<CodeEditorPlaceholder
					actions={[openAnyway]}
					message={intl.formatMessage(codeEditorMessages.binary)}
				/>
			);
		case 'too-large':
			return state.lifted ? (
				<CodeEditorPlaceholder
					actions={[]}
					message={intl.formatMessage(codeEditorMessages.unreadable, { name })}
				/>
			) : (
				<CodeEditorPlaceholder
					actions={[openAnyway]}
					message={intl.formatMessage(codeEditorMessages.tooLarge)}
				/>
			);
		case 'error':
			return <CodeEditorErrorPlaceholder error={state.error} onRetry={() => document.retry()} />;
	}
}
