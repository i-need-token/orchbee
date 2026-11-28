import type {
	ActionExecution,
	ActionInputValidation,
	ActionInvocation,
	ActionSignature,
} from '@orchbee/actions';

import type { FileResource } from '../file-session';

/**
 * The overlay's Actions that other features run: the views, terminal links and pickers open files
 * through them, never through the overlay's state. The Workbench registers them for the shown
 * Workspace's overlay while the window offers the File Bar.
 */
export const FILE_OVERLAY_ACTION_IDS = {
	openFile: 'file-overlay.file.open',
} as const;

/**
 * A file to show in the shown Workspace's overlay, and how it opens. Callers name it through the
 * catalog, which types the open Action's input.
 */
interface FileOverlayOpenInput {
	/**
	 * Gives the active tab the keys once the overlay shows the file, as VS Code focuses the editor it
	 * opens; false leaves the keys where they are, as VS Code's `preserveFocus` does.
	 */
	readonly focus: boolean;
	/**
	 * Opens a preview tab, which the next preview open replaces, as a single click in a view does;
	 * false opens a pinned tab, or pins the file's preview tab.
	 */
	readonly preview: boolean;
	/** The file as the Workspace's file session names it: its path and the key it compares by. */
	readonly resource: FileResource;
	/** The Workspace whose overlay opens the file: the shown one, or the Action does nothing. */
	readonly workspaceId: string;
}

export interface FileOverlayActionCatalog {
	readonly [FILE_OVERLAY_ACTION_IDS.openFile]: ActionSignature<FileOverlayOpenInput>;
}

/**
 * Runs the open Action through the adapter that registered it. The actions runtime refuses a
 * catalog's Action to any other adapter, one a caller makes for this catalog too, so the Workbench,
 * which registers it, hands this to each feature that opens files.
 */
export type FileOverlayOpenFile = (
	input: FileOverlayOpenInput,
	invocation: ActionInvocation,
) => ActionExecution<void>;

function isRecord(input: unknown): input is Readonly<Record<string, unknown>> {
	return typeof input === 'object' && input !== null && !Array.isArray(input);
}

function isName(value: unknown): value is string {
	return typeof value === 'string' && value.length > 0;
}

function fileResource(input: unknown): FileResource | null {
	if (!isRecord(input)) return null;
	const { key, path } = input;
	return isName(key) && isName(path) ? { key, path } : null;
}

export const fileOverlayOpenInput: ActionInputValidation<FileOverlayOpenInput> = {
	name: 'FileOverlayOpenInput',
	parse: (input) => {
		if (!isRecord(input)) return { code: 'expected-file-overlay-open-input', success: false };
		const { focus, preview, workspaceId } = input;
		const resource = fileResource(input['resource']);
		return resource !== null &&
			isName(workspaceId) &&
			typeof focus === 'boolean' &&
			typeof preview === 'boolean'
			? { data: { focus, preview, resource, workspaceId }, success: true }
			: { code: 'expected-file-overlay-open-input', success: false };
	},
};
