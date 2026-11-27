import { useRef, useSyncExternalStore, type ReactNode } from 'react';

import type { CodeEditorTextDocument } from '../../CodeEditorTextDocument';
import type { CodeEditorViewStates } from '../../CodeEditorViewStates';
import type { editor } from '../../monaco-entry';
import { CodeEditorDocumentPlaceholder } from '../code-editor-document-placeholder/CodeEditorDocumentPlaceholder';
import styles from './CodeEditorDiffView.module.css';
import { useDiffEditor } from './useDiffEditor';

interface CodeEditorDiffViewProps {
	/** The right side, such as the working tree's file, which a text tab of that file shares. */
	readonly modified: CodeEditorTextDocument<editor.ITextModel>;
	/** The file's name, which a refusal quotes. */
	readonly name: string;
	/** The left side, such as the file at `HEAD` or in the index. */
	readonly original: CodeEditorTextDocument<editor.ITextModel>;
	/** Monaco, from what `loadCodeEditor` resolves to. */
	readonly runtime: { readonly editor: Pick<typeof editor, 'createDiffEditor'> };
	/** The key the diff's view state is kept under. */
	readonly viewStateKey: string;
	readonly viewStates: CodeEditorViewStates<editor.IDiffEditorViewState>;
}

/**
 * A diff tab's content: both sides in the diff editor once both read as text, or the placeholder
 * of the side that did not, left first. The editor stays mounted behind a placeholder, so the next
 * diff reuses it.
 */
export function CodeEditorDiffView({
	modified,
	name,
	original,
	runtime,
	viewStateKey,
	viewStates,
}: CodeEditorDiffViewProps): ReactNode {
	const hostReference = useRef<HTMLDivElement>(null);
	const originalState = useSyncExternalStore(original.subscribe, original.getState);
	const modifiedState = useSyncExternalStore(modified.subscribe, modified.getState);
	const models =
		originalState.kind === 'text' && modifiedState.kind === 'text'
			? { modified: modifiedState.model, original: originalState.model }
			: null;
	useDiffEditor({ host: hostReference, models, runtime, viewStateKey, viewStates });
	const refusing =
		originalState.kind === 'loading' || originalState.kind === 'text'
			? { document: modified, state: modifiedState }
			: { document: original, state: originalState };

	return (
		<div className={styles['root']} data-slot='code-editor-diff-view'>
			<div className={styles['editor']} hidden={models === null} ref={hostReference} />
			<CodeEditorDocumentPlaceholder
				document={refusing.document}
				name={name}
				state={refusing.state}
			/>
		</div>
	);
}
