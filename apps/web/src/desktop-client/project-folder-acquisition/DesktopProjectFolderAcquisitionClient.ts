import {
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
	DesktopProjectFolderAcquisitionRequestIdSchema,
	type DesktopProjectFolderAcquisitionRendererCommand,
	type DesktopProjectFolderAcquisitionRequestId,
	type DesktopProjectFolderAcquisitionResult,
} from '@orchbee/desktop-client-protocol';
import type {
	WorkbenchProjectCreationReceipt,
	WorkbenchProjectFolderAcquisition,
	WorkbenchProjectFolderAcquisitionFailureCode,
	WorkbenchProjectFolderAcquisitionResult,
	WorkbenchNewProjectFolderCapability,
	WorkbenchNewProjectFolderParent,
	WorkbenchNewProjectFolderParentResult,
	WorkbenchNewProjectFolderResult,
} from '@orchbee/workbench';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';

type ClientResult =
	| DesktopProjectFolderAcquisitionResult
	| { readonly kind: 'failed'; readonly code: WorkbenchProjectFolderAcquisitionFailureCode }
	| { readonly kind: 'outcome-unknown' };

type Operation = 'selection' | 'parent' | 'creation';

interface PendingSelection {
	readonly operation: Operation;
	readonly requestId: DesktopProjectFolderAcquisitionRequestId;
	resolve(result: ClientResult): void;
}

export class DesktopProjectFolderAcquisitionClient implements WorkbenchProjectFolderAcquisition {
	readonly #createRequestId: () => DesktopProjectFolderAcquisitionRequestId;
	readonly #confirmProject: (receipt: WorkbenchProjectCreationReceipt) => Promise<boolean>;
	readonly #lifetime = new AbortController();
	#pending: PendingSelection | undefined;
	#parent: { readonly value: WorkbenchNewProjectFolderParent; attempt: number } | undefined;
	#unacknowledged: string | undefined;
	#unacknowledgedParent: string | undefined;
	readonly #port: MessagePort;
	#sequence = 0;

	readonly newFolder: WorkbenchNewProjectFolderCapability = {
		signal: this.#lifetime.signal,
		acknowledge: (requestId) => this.#acknowledge(requestId),
		confirmProject: async (receipt) =>
			!this.#lifetime.signal.aborted &&
			(await this.#confirmProject(receipt)) &&
			!this.#lifetime.signal.aborted,
		create: (parent, name) => this.#create(parent, name),
		selectParent: () => this.#selectParent(),
		releaseParent: (parent) => this.#releaseParent(parent),
	};

	constructor(
		port: MessagePort,
		createRequestId: () => string = defaultRequestId,
		confirmProject: (receipt: WorkbenchProjectCreationReceipt) => Promise<boolean> = async () =>
			false,
	) {
		this.#port = port;
		this.#confirmProject = confirmProject;
		this.#createRequestId = () =>
			DesktopProjectFolderAcquisitionRequestIdSchema.parse(
				`${createRequestId()}:${++this.#sequence}`,
			);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.addEventListener('close', this.#handleMessageError);
		port.start();
	}

	async selectExistingFolder(): Promise<WorkbenchProjectFolderAcquisitionResult> {
		const result = await this.#request(
			{ requestId: this.#createRequestId(), type: 'desktop-project-folder-selection-requested' },
			'selection',
		);
		return result.kind === 'selected' || result.kind === 'cancelled' || result.kind === 'failed'
			? result
			: { code: 'control-unavailable', kind: 'failed' };
	}

	async #selectParent(): Promise<WorkbenchNewProjectFolderParentResult> {
		const requestId = this.#createRequestId();
		const result = await this.#request(
			{ requestId, type: 'desktop-project-folder-parent-requested' },
			'parent',
		);
		if (result.kind === 'parent-selected' && !this.#lifetime.signal.aborted) {
			const parent = { path: result.path, selectionId: requestId };
			this.#parent = { attempt: 0, value: parent };
			return { kind: 'parent-selected', parent };
		}
		return result.kind === 'cancelled' || result.kind === 'failed'
			? result
			: { code: 'control-unavailable', kind: 'failed' };
	}

	async #create(
		parent: WorkbenchNewProjectFolderParent,
		name: string,
	): Promise<WorkbenchNewProjectFolderResult> {
		const selected = this.#parent;
		if (this.#lifetime.signal.aborted) return { code: 'control-unavailable', kind: 'failed' };
		if (this.#pending !== undefined || this.#unacknowledged !== undefined)
			return { code: 'busy', kind: 'failed' };
		if (
			selected === undefined ||
			parent.selectionId !== selected.value.selectionId ||
			parent.path !== selected.value.path
		) {
			return { code: 'stale-parent', kind: 'failed' };
		}
		const requestId = this.#createRequestId();
		const result = await this.#request(
			{
				attempt: ++selected.attempt,
				name,
				parentRequestId: DesktopProjectFolderAcquisitionRequestIdSchema.parse(parent.selectionId),
				requestId,
				type: 'desktop-project-folder-creation-requested',
			},
			'creation',
		);
		if (result.kind === 'failed') return result;
		if (this.#parent === selected) {
			this.#parent = undefined;
			this.#unacknowledged = requestId;
			this.#unacknowledgedParent = parent.selectionId;
		}
		if (result.kind === 'created' || result.kind === 'created-unavailable')
			return { ...result, requestId };
		return { kind: 'outcome-unknown', requestId };
	}

	#releaseParent(parent: WorkbenchNewProjectFolderParent): void {
		if (this.#parent?.value.selectionId === parent.selectionId) this.#parent = undefined;
		if (this.#unacknowledgedParent === parent.selectionId) {
			this.#unacknowledged = undefined;
			this.#unacknowledgedParent = undefined;
		}
		if (this.#lifetime.signal.aborted) return;
		try {
			this.#port.postMessage(
				DesktopProjectFolderAcquisitionRendererCommandSchema.parse({
					requestId: parent.selectionId,
					type: 'desktop-project-folder-parent-released',
				}),
			);
		} catch {
			this.#retire();
		}
	}

	#acknowledge(requestId: string): void {
		if (this.#unacknowledged !== requestId || this.#lifetime.signal.aborted) return;
		try {
			this.#port.postMessage(
				DesktopProjectFolderAcquisitionRendererCommandSchema.parse({
					requestId,
					type: 'desktop-project-folder-creation-acknowledged',
				}),
			);
			this.#unacknowledged = undefined;
			this.#unacknowledgedParent = undefined;
		} catch {
			this.#retire();
		}
	}

	#request(
		command: DesktopProjectFolderAcquisitionRendererCommand,
		operation: Operation,
	): Promise<ClientResult> {
		if (this.#lifetime.signal.aborted)
			return Promise.resolve({ code: 'control-unavailable', kind: 'failed' });
		if (this.#pending !== undefined || this.#unacknowledged !== undefined)
			return Promise.resolve({ code: 'busy', kind: 'failed' });
		if (operation !== 'creation') this.#parent = undefined;
		const result = new Promise<ClientResult>((resolve) => {
			this.#pending = { operation, requestId: command.requestId, resolve };
		});
		try {
			this.#port.postMessage(DesktopProjectFolderAcquisitionRendererCommandSchema.parse(command));
		} catch {
			this.#retire();
		}
		return result;
	}

	dispose(): void {
		this.#retire();
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#lifetime.signal.aborted) return;
		const parsed = DesktopProjectFolderAcquisitionMainEventSchema.safeParse(event.data);
		const pending = this.#pending;
		if (
			event.ports.length !== 0 ||
			!parsed.success ||
			pending === undefined ||
			parsed.data.requestId !== pending.requestId
		) {
			this.#retire();
			return;
		}
		if (parsed.data.type === 'desktop-project-folder-acquisition-failed') {
			this.#pending = undefined;
			pending.resolve({ code: parsed.data.code, kind: 'failed' });
			return;
		}
		const result = parsed.data.result;
		const valid =
			pending.operation === 'creation'
				? result.kind === 'created' ||
					result.kind === 'created-unavailable' ||
					result.kind === 'creation-unknown'
				: result.kind === 'cancelled' ||
					(pending.operation === 'parent'
						? result.kind === 'parent-selected'
						: result.kind === 'selected');
		if (!valid) {
			this.#retire();
			return;
		}
		this.#pending = undefined;
		pending.resolve(result);
	};

	readonly #handleMessageError = (): void => this.#retire();

	#retire(): void {
		if (this.#lifetime.signal.aborted) return;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		this.#port.removeEventListener('close', this.#handleMessageError);
		this.#port.close();
		const pending = this.#pending;
		this.#pending = undefined;
		this.#parent = undefined;
		pending?.resolve(
			pending.operation === 'creation'
				? { kind: 'outcome-unknown' }
				: { code: 'control-unavailable', kind: 'failed' },
		);
		this.#lifetime.abort();
	}
}

function defaultRequestId(): string {
	return `project-folder:${createBrowserRandomIdentifier()}`;
}
