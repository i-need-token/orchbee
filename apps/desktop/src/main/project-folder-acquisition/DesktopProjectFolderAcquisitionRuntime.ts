import type { DesktopProjectFolderAcquisitionResult } from '@orchbee/desktop-client-protocol';
import { dialog, type BrowserWindow, type OpenDialogOptions } from 'electron';

import type { DesktopProjectFolderAcquisitionBrokerResult } from './DesktopProjectFolderAcquisitionBroker';
import {
	resolveSelectedProjectFolder,
	type SelectedProjectFolderValidationResult,
} from './resolveSelectedProjectFolder';

interface NativeFolderSelection {
	readonly canceled: boolean;
	readonly filePaths: readonly string[];
}

export interface DesktopProjectFolderAcquisitionRuntimeDependencies {
	readonly resolveSelectedFolder: (path: string) => Promise<SelectedProjectFolderValidationResult>;
	readonly showOpenDialog: (
		window: BrowserWindow,
		options: OpenDialogOptions,
	) => Promise<NativeFolderSelection>;
}

const defaultDependencies: DesktopProjectFolderAcquisitionRuntimeDependencies = {
	resolveSelectedFolder: resolveSelectedProjectFolder,
	showOpenDialog: (window, options) => dialog.showOpenDialog(window, options),
};

export class DesktopProjectFolderAcquisitionRuntime {
	readonly #dependencies: DesktopProjectFolderAcquisitionRuntimeDependencies;
	readonly #window: BrowserWindow;

	constructor(
		window: BrowserWindow,
		dependencies: DesktopProjectFolderAcquisitionRuntimeDependencies = defaultDependencies,
	) {
		this.#window = window;
		this.#dependencies = dependencies;
	}

	async selectExistingFolder(): Promise<DesktopProjectFolderAcquisitionBrokerResult> {
		let nativeSelection: NativeFolderSelection;
		try {
			nativeSelection = await this.#dependencies.showOpenDialog(this.#window, {
				properties: ['openDirectory'],
			});
		} catch {
			return { code: 'dialog-failed', kind: 'failed' };
		}
		if (nativeSelection.canceled) {
			return { kind: 'cancelled' } satisfies DesktopProjectFolderAcquisitionResult;
		}
		const selectedPath = nativeSelection.filePaths[0];
		if (nativeSelection.filePaths.length !== 1 || selectedPath === undefined) {
			return { code: 'validation-failed', kind: 'failed' };
		}
		return this.#dependencies.resolveSelectedFolder(selectedPath);
	}
}
