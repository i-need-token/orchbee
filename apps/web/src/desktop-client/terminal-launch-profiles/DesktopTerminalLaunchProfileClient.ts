import {
	DesktopTerminalLaunchProfileMainEventSchema,
	DesktopTerminalLaunchProfileRendererCommandSchema,
	DesktopTerminalLaunchProfileRequestIdSchema,
	type DesktopTerminalLaunchProfile,
	type DesktopTerminalLaunchProfileId,
	type DesktopTerminalLaunchProfileMainEvent,
	type DesktopTerminalLaunchProfileRendererCommand,
	type DesktopTerminalLaunchProfileRequestId,
	type DesktopTerminalLaunchProfileSnapshot,
} from '@orchbee/desktop-client-protocol';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';
import type {
	DesktopTerminalLaunchProfileClientInitialization,
	DesktopTerminalLaunchProfileClientOperationResult,
	DesktopTerminalLaunchProfileClientSnapshot,
	DesktopTerminalLaunchProfileControl,
} from '../../terminal-launch-profile-control';

type DesktopTerminalLaunchProfileProtocolResult = DesktopTerminalLaunchProfileMainEvent['result'];
type CommandWithoutRequestId = DesktopTerminalLaunchProfileRendererCommand extends infer Command
	? Command extends DesktopTerminalLaunchProfileRendererCommand
		? Omit<Command, 'requestId'>
		: never
	: never;

interface PendingRequest {
	readonly requestId: DesktopTerminalLaunchProfileRequestId;
	resolve(result: DesktopTerminalLaunchProfileProtocolResult | null): void;
}

export class DesktopTerminalLaunchProfileClient implements DesktopTerminalLaunchProfileControl {
	readonly #createRequestId: () => DesktopTerminalLaunchProfileRequestId;
	#disposed = false;
	#initialization: Promise<DesktopTerminalLaunchProfileClientInitialization> | undefined;
	readonly #listeners = new Set<() => void>();
	#pending: PendingRequest | undefined;
	readonly #port: MessagePort;
	#sequence = 0;
	#snapshot: DesktopTerminalLaunchProfileClientSnapshot | undefined;

	constructor(port: MessagePort, createRequestId: () => string = defaultRequestId) {
		this.#port = port;
		this.#createRequestId = () =>
			DesktopTerminalLaunchProfileRequestIdSchema.parse(`${createRequestId()}:${++this.#sequence}`);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	initialize(): Promise<DesktopTerminalLaunchProfileClientInitialization> {
		this.#initialization ??= this.#initialize();
		return this.#initialization;
	}

	async #initialize(): Promise<DesktopTerminalLaunchProfileClientInitialization> {
		if (this.#snapshot !== undefined) return { kind: 'ready' };
		const result = await this.#request({
			type: 'desktop-terminal-launch-profile-snapshot-requested',
		});
		return result === null ? { kind: 'unavailable' } : { kind: 'ready' };
	}

	getSnapshot = (): DesktopTerminalLaunchProfileClientSnapshot => {
		if (this.#snapshot === undefined) {
			throw new Error('Terminal launch profiles must initialize before reading their snapshot.');
		}
		return this.#snapshot;
	};

	subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	createProfile(
		name: string,
		sourceProfileId: DesktopTerminalLaunchProfileId,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult> {
		return this.#operate({
			name,
			sourceProfileId,
			type: 'desktop-terminal-launch-profile-create-requested',
		});
	}

	updateProfile(
		profile: DesktopTerminalLaunchProfile,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult> {
		return this.#operate({
			profile,
			type: 'desktop-terminal-launch-profile-update-requested',
		});
	}

	selectDefault(
		profileId: DesktopTerminalLaunchProfileId,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult> {
		return this.#operate({
			profileId,
			type: 'desktop-terminal-launch-profile-default-requested',
		});
	}

	deleteProfile(
		profileId: DesktopTerminalLaunchProfileId,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult> {
		return this.#operate({
			profileId,
			type: 'desktop-terminal-launch-profile-delete-requested',
		});
	}

	reset(): Promise<DesktopTerminalLaunchProfileClientOperationResult> {
		return this.#operate({ type: 'desktop-terminal-launch-profile-reset-requested' });
	}

	dispose(): void {
		this.#retire(true);
	}

	async #operate(
		command: CommandWithoutRequestId,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult> {
		if (this.#snapshot === undefined || this.#disposed) return { kind: 'unavailable' };
		if (this.#pending !== undefined) {
			return {
				code: 'busy',
				kind: 'failed',
				snapshot: profileSnapshot(this.#snapshot),
			};
		}
		const result = await this.#request(command);
		return result ?? { kind: 'unavailable' };
	}

	#request(
		command: CommandWithoutRequestId,
	): Promise<DesktopTerminalLaunchProfileProtocolResult | null> {
		if (this.#disposed || this.#pending !== undefined) return Promise.resolve(null);
		const requestId = this.#createRequestId();
		const message = DesktopTerminalLaunchProfileRendererCommandSchema.parse({
			...command,
			requestId,
		});
		const result = new Promise<DesktopTerminalLaunchProfileProtocolResult | null>((resolve) => {
			this.#pending = { requestId, resolve };
		});
		this.#updateState({ pending: true });
		try {
			this.#port.postMessage(message);
		} catch {
			this.#retire(true);
		}
		return result;
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopTerminalLaunchProfileMainEventSchema.safeParse(event.data);
		const pending = this.#pending;
		if (!parsed.success || pending === undefined || parsed.data.requestId !== pending.requestId) {
			this.#retire(true);
			return;
		}
		this.#pending = undefined;
		const result = parsed.data.result;
		this.#snapshot = {
			...result.snapshot,
			failureCode: result.kind === 'failed' ? result.code : null,
			pending: false,
		};
		this.#emit();
		pending.resolve(result);
	};

	readonly #handleMessageError = (): void => {
		this.#retire(true);
	};

	#updateState(patch: Pick<DesktopTerminalLaunchProfileClientSnapshot, 'pending'>): void {
		if (this.#snapshot === undefined) return;
		this.#snapshot = {
			...this.#snapshot,
			...patch,
			...(patch.pending ? { failureCode: null } : {}),
		};
		this.#emit();
	}

	#emit(): void {
		for (const listener of this.#listeners) listener();
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		if (closePort) this.#port.close();
		const pending = this.#pending;
		this.#pending = undefined;
		if (this.#snapshot !== undefined) {
			this.#snapshot = {
				...this.#snapshot,
				failureCode: 'control-unavailable',
				pending: false,
			};
			this.#emit();
		}
		pending?.resolve(null);
	}
}

function profileSnapshot(
	snapshot: DesktopTerminalLaunchProfileClientSnapshot,
): DesktopTerminalLaunchProfileSnapshot {
	return {
		capabilities: snapshot.capabilities,
		catalog: snapshot.catalog,
		persistenceIssue: snapshot.persistenceIssue,
		source: snapshot.source,
	};
}

function defaultRequestId(): string {
	return `terminal-profile:${createBrowserRandomIdentifier()}`;
}
