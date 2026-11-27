import { ProductIcon } from '@orchbee/ui/components';
import { useLayoutEffect, useRef, useSyncExternalStore, type ReactNode } from 'react';

import type {
	CodeEditorTextDocument,
	CodeEditorTextDocumentState,
} from '../../../../CodeEditorTextDocument';
import type { editor } from '../../../../monaco-entry';
import { CodeEditorDocumentPlaceholder } from '../../../code-editor-document-placeholder/CodeEditorDocumentPlaceholder';
import {
	MULTI_DIFF_BOTTOM_PADDING,
	MULTI_DIFF_HEADER_HEIGHT,
	MULTI_DIFF_PLACEHOLDER_HEIGHT,
} from '../../constants/multi-diff-geometry';
import type { MultiDiffController } from '../../MultiDiffController';
import styles from './MultiDiffItem.module.css';
import { useMultiDiffItemEditor } from './useMultiDiffItemEditor';

type DocumentState = CodeEditorTextDocumentState<editor.ITextModel>;

interface MultiDiffSide {
	readonly document: CodeEditorTextDocument<editor.ITextModel>;
	readonly state: DocumentState;
}

interface MultiDiffItemProps {
	readonly controller: MultiDiffController;
	readonly runtime: { readonly editor: Pick<typeof editor, 'createDiffEditor'> };
	readonly slot: number;
}

const subscribeToNothing = (): (() => void) => () => undefined;
const readNothing = (): null => null;

/** The side whose placeholder shows in place of the diff, left first, as in the diff view. */
function refusingSide(
	original: MultiDiffSide | null,
	modified: MultiDiffSide | null,
): { readonly document: MultiDiffSide['document']; readonly state: DocumentState } | null {
	if (original === null || modified === null) return null;
	if (original.state.kind === 'loading' || original.state.kind === 'text') return modified;
	return original;
}

/**
 * One slot of the multi-diff's pool: the file it shows under a header that collapses it, with the
 * file's diff, or the placeholder of the side that does not read as text, left first. A slot that
 * shows no file stays mounted and hidden, keeping its editor for the next file.
 */
export function MultiDiffItem({ controller, runtime, slot }: MultiDiffItemProps): ReactNode {
	const shown = useSyncExternalStore(controller.subscribe, () => controller.slot(slot));
	const hostReference = useRef<HTMLDivElement>(null);
	const entry = shown?.entry ?? null;
	const collapsed = shown?.collapsed ?? false;
	const original = entry?.original ?? null;
	const modified = entry?.modified ?? null;
	const originalState: DocumentState | null = useSyncExternalStore(
		original?.subscribe ?? subscribeToNothing,
		original?.getState ?? readNothing,
	);
	const modifiedState: DocumentState | null = useSyncExternalStore(
		modified?.subscribe ?? subscribeToNothing,
		modified?.getState ?? readNothing,
	);
	const sides =
		!collapsed && originalState?.kind === 'text' && modifiedState?.kind === 'text'
			? { modified: modifiedState.model, original: originalState.model }
			: null;
	const refusing = refusingSide(
		original === null || originalState === null
			? null
			: { document: original, state: originalState },
		modified === null || modifiedState === null
			? null
			: { document: modified, state: modifiedState },
	);
	const placeholder = sides === null && refusing !== null && refusing.state.kind !== 'text';
	const fileKey = entry?.key ?? null;
	useMultiDiffItemEditor({
		controller,
		editorHeight: (shown?.frameHeight ?? 0) - MULTI_DIFF_HEADER_HEIGHT - MULTI_DIFF_BOTTOM_PADDING,
		fileKey,
		host: hostReference,
		runtime,
		sides,
		slot,
		width: shown?.width ?? 0,
	});
	// A placeholder has its fixed height; a loading file keeps its estimate until it reads.
	const settledPlaceholder = placeholder && refusing?.state.kind !== 'loading';
	useLayoutEffect(() => {
		if (fileKey !== null && !collapsed && settledPlaceholder) {
			controller.setContentHeight(fileKey, MULTI_DIFF_PLACEHOLDER_HEIGHT);
		}
	}, [collapsed, controller, fileKey, settledPlaceholder]);

	return (
		<div
			className={styles['item']}
			data-slot='code-editor-multi-diff-item'
			hidden={shown === null}
			style={{
				'--ob-comp-code-editor-multi-diff-frame-height': `${shown?.frameHeight ?? 0}px`,
				'--ob-comp-code-editor-multi-diff-item-height': `${shown?.height ?? 0}px`,
				'--ob-comp-code-editor-multi-diff-item-start': `${shown?.top ?? 0}px`,
			}}
		>
			<div className={styles['frame']}>
				<button
					aria-expanded={!collapsed}
					className={styles['header']}
					onClick={() => {
						if (fileKey !== null) controller.toggle(fileKey);
					}}
					type='button'
				>
					<span className={styles['twistie']} data-open={collapsed ? undefined : ''}>
						<ProductIcon name='chevron-right' size='disclosure' />
					</span>
					<span className={styles['name']}>{entry?.name}</span>
					<span className={styles['description']}>{entry?.description}</span>
				</button>
				<div className={styles['body']} hidden={collapsed}>
					<div className={styles['editor']} hidden={sides === null} ref={hostReference} />
					{placeholder && refusing !== null && entry !== null ? (
						<CodeEditorDocumentPlaceholder
							document={refusing.document}
							name={entry.name}
							state={refusing.state}
						/>
					) : null}
				</div>
			</div>
		</div>
	);
}
