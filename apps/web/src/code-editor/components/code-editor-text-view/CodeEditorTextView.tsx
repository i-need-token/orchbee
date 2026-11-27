import { useRef, useSyncExternalStore, type ReactNode } from 'react';

import type { CodeEditorTextDocument } from '../../CodeEditorTextDocument';
import type { CodeEditorViewStates } from '../../CodeEditorViewStates';
import type { editor } from '../../monaco-entry';
import { CodeEditorDocumentPlaceholder } from '../code-editor-document-placeholder/CodeEditorDocumentPlaceholder';
import styles from './CodeEditorTextView.module.css';
import { useTextEditor } from './useTextEditor';

interface CodeEditorTextViewProps {
	readonly document: CodeEditorTextDocument<editor.ITextModel>;
	/** The file's name: the editor's accessible name, and the name a refusal quotes. */
	readonly name: string;
	/** Monaco, from what `loadCodeEditor` resolves to. */
	readonly runtime: { readonly editor: Pick<typeof editor, 'create'> };
	/** The key the file's view state is kept under. */
	readonly viewStateKey: string;
	readonly viewStates: CodeEditorViewStates<editor.ICodeEditorViewState>;
}

/**
 * A text tab's content: the file in the text editor, or the placeholder that says why it is not
 * there. The editor stays mounted behind a placeholder, so the next text document reuses it.
 */
export function CodeEditorTextView({
	document,
	name,
	runtime,
	viewStateKey,
	viewStates,
}: CodeEditorTextViewProps): ReactNode {
	const hostReference = useRef<HTMLDivElement>(null);
	const state = useSyncExternalStore(document.subscribe, document.getState);
	const model = state.kind === 'text' ? state.model : null;
	useTextEditor({ host: hostReference, model, name, runtime, viewStateKey, viewStates });

	return (
		<div className={styles['root']} data-slot='code-editor-text-view'>
			<div className={styles['editor']} hidden={model === null} ref={hostReference} />
			<CodeEditorDocumentPlaceholder document={document} name={name} state={state} />
		</div>
	);
}
