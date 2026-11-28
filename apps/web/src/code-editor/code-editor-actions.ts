/**
 * The editor font zoom's Actions, which the Workbench registers once for every editor, since the
 * zoom is page-wide. VS Code parity: editor.action.fontZoomIn, editor.action.fontZoomOut and
 * editor.action.fontZoomReset.
 */
export const CODE_EDITOR_ACTION_IDS = {
	fontZoomIn: 'code-editor.font-zoom-in',
	fontZoomOut: 'code-editor.font-zoom-out',
	fontZoomReset: 'code-editor.font-zoom-reset',
} as const;

/**
 * The Action that runs Monaco's command `monacoId`, which is VS Code's command of that identity, in
 * the editor that registers it: the one with the keys, or the one that had them last.
 */
export function codeEditorCommandActionId(monacoId: string): string {
	return `code-editor.command.${monacoId}`;
}
