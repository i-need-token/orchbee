import { spawnSync } from 'node:child_process';

import { type DesktopPathRefusal, interpretPathRefusal } from '../server-connection';

// Cross-runtime contract: the bundled Client's hidden helper, permanent under this name.
const CREATE_WINDOW_FOLDER_HELPER = '--create-window-folder';
/** Creating a few folders takes moments; this only bounds a Client that stopped answering. */
const HELPER_DEADLINE_MILLISECONDS = 20_000;

/**
 * The bundled Client did not create the data root and Desktop's folder, so nothing in the root was
 * written by Main. `refusal` names the item the Client refused, when it named one.
 */
export class DesktopFolderNotCreated extends Error {
	readonly refusal: DesktopPathRefusal | undefined;
	readonly root: string;

	constructor(root: string, refusal: DesktopPathRefusal | undefined) {
		super('The bundled Client did not create the Desktop folder.');
		this.name = 'DesktopFolderNotCreated';
		this.refusal = refusal;
		this.root = root;
	}
}

export interface DesktopFolderHelper {
	/** Main's environment, so the Client derives the same data root. */
	readonly environment: NodeJS.ProcessEnv;
	readonly executable: string;
	readonly platform: NodeJS.Platform;
	readonly workingDirectory: string;
}

/**
 * Has the bundled Client create the data root `root`, `internal/` and Desktop's folder under the
 * rule every Orchbee folder follows, so the Session never meets a folder Main made that it would
 * refuse; throws {@link DesktopFolderNotCreated} when the Client does not.
 */
export function createDesktopFolder(helper: DesktopFolderHelper, root: string): void {
	const answer = spawnSync(helper.executable, [CREATE_WINDOW_FOLDER_HELPER], {
		cwd: helper.workingDirectory,
		encoding: 'utf8',
		env: helper.environment,
		timeout: HELPER_DEADLINE_MILLISECONDS,
		windowsHide: true,
	});
	if (answer.status === 0) return;
	let document: unknown;
	try {
		document = JSON.parse(answer.stdout);
	} catch {
		document = undefined;
	}
	throw new DesktopFolderNotCreated(root, interpretPathRefusal(document, helper.platform));
}
