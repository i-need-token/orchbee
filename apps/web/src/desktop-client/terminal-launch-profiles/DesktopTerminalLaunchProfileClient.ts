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
import type { UserPreferencesControl } from '../../user-preferences';
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
	readonly #removeUserPreferences: (() => void) | undefined;
	#refreshNeeded = false;

	constructor(
		port: MessagePort,
		createRequestId: () => string = defaultRequestId,
		userPreferences?: UserPreferencesControl,
	) {
		this.#port = port;
		this.#createRequestId = () =>
			DesktopTerminalLaunchProfileRequestIdSchema.parse(`${createRequestId()}:${++this.#sequence}`);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
		let source = userPreferences?.getSnapshot().snapshot?.source;
		this.#removeUserPreferences = userPreferences?.subscribe(() => {
			const next = userPreferences.getSnapshot().snapshot?.source;
			if (next === source) return;
			source = next;
			this.#refreshNeeded = true;
			this.#refresh();
		});
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
		if (this.#disposed) return () => undefined;
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
		preferencesSource?: string,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult> {
		return this.#operate({
			profile,
			...(preferencesSource === undefined ? {} : { preferencesSource }),
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
			...(this.#snapshot?.preferencesSource === undefined
				? {}
				: { preferencesSource: this.#snapshot.preferencesSource }),
			...command,
			requestId,
		});
		const result = new Promise<DesktopTerminalLaunchProfileProtocolResult | null>((resolve) => {
			this.#pending = { requestId, resolve };
		});
		this.#updateState({ pending: true });
		if (this.#disposed) return result;
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
		this.#refresh();
	};

	#refresh(): void {
		if (
			!this.#refreshNeeded ||
			this.#disposed ||
			this.#pending !== undefined ||
			this.#snapshot === undefined
		)
			return;
		this.#refreshNeeded = false;
		void this.#request({ type: 'desktop-terminal-launch-profile-snapshot-requested' });
	}

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
		for (const listener of new Set(this.#listeners)) {
			if (!this.#listeners.has(listener)) continue;
			try {
				listener();
			} catch {
				// Presentation observers cannot strand a request or its retirement receipt.
			}
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeUserPreferences?.();
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
		this.#listeners.clear();
		pending?.resolve(null);
	}
}

function profileSnapshot(
	snapshot: DesktopTerminalLaunchProfileClientSnapshot,
): DesktopTerminalLaunchProfileSnapshot {
	return {
		capabilities: snapshot.capabilities,
		...(snapshot.preferencesSource === undefined
			? {}
			: { preferencesSource: snapshot.preferencesSource }),
		catalog: snapshot.catalog,
		persistenceIssue: snapshot.persistenceIssue,
		source: snapshot.source,
	};
}

function defaultRequestId(): string {
	return `terminal-profile:${createBrowserRandomIdentifier()}`;
}
