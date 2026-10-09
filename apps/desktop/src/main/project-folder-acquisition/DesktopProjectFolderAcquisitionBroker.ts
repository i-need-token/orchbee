import {
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
	type DesktopProjectFolderAcquisitionFailureCode,
	type DesktopProjectFolderAcquisitionRendererCommand,
	type DesktopProjectFolderAcquisitionRequestId,
	type DesktopProjectFolderAcquisitionResult,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

import type { NewProjectFolderCreationResult } from './createProjectFolder';

export type DesktopProjectFolderAcquisitionBrokerResult =
	| DesktopProjectFolderAcquisitionResult
	| { readonly code: DesktopProjectFolderAcquisitionFailureCode; readonly kind: 'failed' };

export interface DesktopProjectFolderAcquisitionBrokerRuntime {
	selectExistingFolder(): Promise<DesktopProjectFolderAcquisitionBrokerResult>;
	createFolder(
		parentPath: string,
		name: string,
		isCurrent: () => boolean,
	): Promise<NewProjectFolderCreationResult>;
	reportUnacknowledgedCreation(path: string, knownCreated: boolean): Promise<void>;
}

interface SelectedParent {
	readonly requestId: DesktopProjectFolderAcquisitionRequestId;
	readonly path: string;
	attempt: number;
}

interface RetainedCreation {
	readonly parentRequestId: DesktopProjectFolderAcquisitionRequestId;
	readonly requestId: DesktopProjectFolderAcquisitionRequestId;
	readonly path: string;
	readonly knownCreated: boolean;
}

export class DesktopProjectFolderAcquisitionBroker {
	// Keep this same owner alive across channel disposal until reporting/ACK, or Main exit.
	static readonly #unreported = new Set<DesktopProjectFolderAcquisitionBroker>();
	#activeRequestId: DesktopProjectFolderAcquisitionRequestId | undefined;
	#activeParentRequestId: DesktopProjectFolderAcquisitionRequestId | undefined;
	#abandonedRequestId: DesktopProjectFolderAcquisitionRequestId | undefined;
	#disposed = false;
	#parent: SelectedParent | undefined;
	#retainedCreation: RetainedCreation | undefined;
	#reporting = false;
	readonly #port: MessagePortMain;
	readonly #runtime: DesktopProjectFolderAcquisitionBrokerRuntime;
	#started = false;

	constructor(port: MessagePortMain, runtime: DesktopProjectFolderAcquisitionBrokerRuntime) {
		this.#port = port;
		this.#runtime = runtime;
	}

	start(): void {
		if (this.#started || this.#disposed) return;
		this.#started = true;
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handleClose);
		this.#port.start();
	}

	dispose(): void {
		this.#retire(true);
	}

	readonly #handleClose = (): void => this.#retire(false);

	readonly #handleMessage = (event: MessageEvent): void => {
		if (this.#disposed) return;
		const parsed = DesktopProjectFolderAcquisitionRendererCommandSchema.safeParse(event.data);
		if (event.ports.length !== 0 || !parsed.success) {
			this.#retire(true);
			return;
		}
		const command = parsed.data;
		const requestId = command.requestId;
		if (command.type === 'desktop-project-folder-parent-released') {
			if (this.#parent?.requestId === requestId) this.#parent = undefined;
			if (this.#activeParentRequestId === requestId)
				this.#abandonedRequestId = this.#activeRequestId;
			if (this.#retainedCreation?.parentRequestId === requestId) this.#reportRetainedCreation();
			return;
		}
		if (command.type === 'desktop-project-folder-creation-acknowledged') {
			if (this.#retainedCreation?.requestId === requestId) {
				this.#retainedCreation = undefined;
				DesktopProjectFolderAcquisitionBroker.#unreported.delete(this);
			}
			return;
		}
		if (
			this.#activeRequestId !== undefined ||
			this.#retainedCreation !== undefined ||
			this.#reporting
		) {
			if (this.#activeRequestId !== requestId) this.#postFailed(requestId, 'busy');
			return;
		}
		let parent: SelectedParent | undefined;
		if (command.type === 'desktop-project-folder-creation-requested') {
			parent = this.#parent;
			if (
				parent === undefined ||
				parent.requestId !== command.parentRequestId ||
				command.attempt !== parent.attempt + 1
			) {
				this.#postFailed(requestId, 'stale-parent');
				return;
			}
			parent.attempt = command.attempt;
			this.#activeParentRequestId = parent.requestId;
		} else {
			this.#parent = undefined;
		}
		this.#activeRequestId = requestId;
		void this.#execute(command, parent).finally(() => {
			if (this.#activeRequestId === requestId) {
				this.#activeRequestId = undefined;
				this.#activeParentRequestId = undefined;
			}
		});
	};

	async #execute(
		command: DesktopProjectFolderAcquisitionRendererCommand,
		parent: SelectedParent | undefined,
	): Promise<void> {
		const requestId = command.requestId;
		let result: DesktopProjectFolderAcquisitionBrokerResult;
		if (command.type === 'desktop-project-folder-creation-requested' && parent !== undefined) {
			try {
				result = await this.#runtime.createFolder(
					parent.path,
					command.name,
					() =>
						!this.#disposed &&
						this.#activeRequestId === requestId &&
						this.#abandonedRequestId !== requestId,
				);
			} catch {
				// An adapter exception cannot prove the filesystem had no effect.
				this.#parent = undefined;
				this.#retire(true);
				return;
			}
			if (
				result.kind === 'created' ||
				result.kind === 'created-unavailable' ||
				result.kind === 'creation-unknown'
			) {
				this.#parent = undefined;
				this.#retainedCreation = {
					knownCreated: result.kind !== 'creation-unknown',
					parentRequestId: parent.requestId,
					path: result.path,
					requestId,
				};
				DesktopProjectFolderAcquisitionBroker.#unreported.add(this);
				if (this.#disposed || this.#abandonedRequestId === requestId)
					this.#reportRetainedCreation();
			}
		} else {
			try {
				result = await this.#runtime.selectExistingFolder();
			} catch {
				result = { code: 'dialog-failed', kind: 'failed' };
			}
			if (
				command.type === 'desktop-project-folder-parent-requested' &&
				result.kind === 'selected' &&
				!this.#disposed
			) {
				this.#parent = { attempt: 0, path: result.path, requestId };
				result = { kind: 'parent-selected', path: result.path };
			}
		}
		if (result.kind === 'failed') this.#postFailed(requestId, result.code);
		else this.#post({ requestId, result, type: 'desktop-project-folder-acquisition-completed' });
	}

	#postFailed(
		requestId: DesktopProjectFolderAcquisitionRequestId,
		code: DesktopProjectFolderAcquisitionFailureCode,
	): void {
		this.#post({ code, requestId, type: 'desktop-project-folder-acquisition-failed' });
	}

	#post(event: Parameters<typeof DesktopProjectFolderAcquisitionMainEventSchema.parse>[0]): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(DesktopProjectFolderAcquisitionMainEventSchema.parse(event));
		} catch {
			this.#retire(true);
		}
	}

	#reportRetainedCreation(): void {
		const receipt = this.#retainedCreation;
		if (receipt === undefined || this.#reporting) return;
		this.#reporting = true;
		void this.#runtime
			.reportUnacknowledgedCreation(receipt.path, receipt.knownCreated)
			.then(() => {
				if (this.#retainedCreation === receipt) {
					this.#retainedCreation = undefined;
					DesktopProjectFolderAcquisitionBroker.#unreported.delete(this);
				}
			})
			.catch(() => {
				// Keep the receipt in memory; a destroyed Main/window has no delivery guarantee.
			})
			.finally(() => {
				this.#reporting = false;
			});
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#parent = undefined;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		if (closePort) this.#port.close();
		this.#reportRetainedCreation();
	}
}
