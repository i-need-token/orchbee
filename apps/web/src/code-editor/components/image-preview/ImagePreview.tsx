import { codeEditorMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { useRef, useSyncExternalStore, type ReactNode } from 'react';

import type { CodeEditorViewStates } from '../../CodeEditorViewStates';
import type { ImagePreviewDocument } from '../../ImagePreviewDocument';
import type { ImageViewerViewState } from '../../ImageViewer';
import { CodeEditorErrorPlaceholder } from '../code-editor-error-placeholder/CodeEditorErrorPlaceholder';
import { CodeEditorPlaceholder } from '../code-editor-placeholder/CodeEditorPlaceholder';
import styles from './ImagePreview.module.css';
import { useImageViewer } from './useImageViewer';

interface ImagePreviewProps {
	readonly document: ImagePreviewDocument;
	/** The file's name: the image's alternative text. */
	readonly name: string;
	/** Reopens the file in the text editor, offered when the content is no image. */
	readonly onOpenAsText: () => void;
	readonly platform: KeyboardShortcutPlatform;
	/** The key the file's zoom and scroll are kept under. */
	readonly viewStateKey: string;
	readonly viewStates: CodeEditorViewStates<ImageViewerViewState>;
}

/**
 * VS Code's image preview: the image over a checkerboard, fitted until zoomed, or the reason it
 * shows none. Content that does not decode keeps the viewer mounted behind VS Code's message, so
 * the next image reuses it.
 */
export function ImagePreview({
	document,
	name,
	onOpenAsText,
	platform,
	viewStateKey,
	viewStates,
}: ImagePreviewProps): ReactNode {
	const intl = useOrchbeeIntl();
	const viewerReference = useRef<HTMLDivElement>(null);
	const imageReference = useRef<HTMLImageElement>(null);
	const state = useSyncExternalStore(document.subscribe, document.getState);
	const undecodable = useImageViewer({
		element: viewerReference,
		image: imageReference,
		platform,
		source: state.kind === 'image' ? state.source : null,
		viewStateKey,
		viewStates,
	});

	let message: ReactNode = null;
	if (state.kind === 'error') {
		message = <CodeEditorErrorPlaceholder error={state.error} onRetry={() => document.retry()} />;
	} else if (undecodable) {
		message = (
			<CodeEditorPlaceholder
				actions={[{ label: intl.formatMessage(codeEditorMessages.openAsText), run: onOpenAsText }]}
				message={intl.formatMessage(codeEditorMessages.imageLoadFailed)}
			/>
		);
	}

	return (
		<div className={styles['root']} data-slot='image-preview'>
			<div
				aria-label={intl.formatMessage(codeEditorMessages.imagePreview, { name })}
				className={styles['viewer']}
				hidden={message !== null}
				ref={viewerReference}
				role='img'
				tabIndex={0}
			>
				<img alt={name} className={styles['image']} ref={imageReference} />
			</div>
			{message}
		</div>
	);
}
