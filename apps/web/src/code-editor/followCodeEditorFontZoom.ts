import type { CodeEditorFontZoom } from './code-editor-font-zoom';
import type { editor } from './monaco-entry';

/**
 * Makes Monaco's page-wide editor zoom follow `zoom` from the first load on, and `zoom` follow a
 * level Monaco sets itself, so either way every editor shows the same level.
 */
export function followCodeEditorFontZoom(
	zoom: CodeEditorFontZoom,
	monaco: { readonly editor: Pick<typeof editor, 'EditorZoom'> },
): void {
	const { EditorZoom } = monaco.editor;
	EditorZoom.setZoomLevel(zoom.getLevel());
	zoom.subscribe(() => EditorZoom.setZoomLevel(zoom.getLevel()));
	EditorZoom.onDidChangeZoomLevel((level) => zoom.setLevel(level));
}
