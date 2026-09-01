import {
	DesktopAgentIntegrationManagementMainEventSchema,
	DesktopAgentIntegrationManagementRendererCommandSchema,
	DesktopAgentIntegrationManagementRequestIdSchema,
	type DesktopAgentIntegrationId,
	type DesktopAgentIntegrationManagementMainEvent,
	type DesktopAgentIntegrationManagementRendererCommand,
	type DesktopAgentIntegrationManagementRequestId,
	type DesktopAgentIntegrationSnapshotItem,
} from '@orchbee/desktop-client-protocol';

import type {
	AgentIntegrationManagementClientInitialization,
	AgentIntegrationManagementClientOperationResult,
	AgentIntegrationManagementClientSnapshot,
	AgentIntegrationManagementControl,
} from '../../agent-integration-management';
import { createBrowserRandomIdentifier } from '../../browser-random-identifier';

type ProtocolResult = Extract<
	DesktopAgentIntegrationManagementMainEvent,
	{ readonly type: 'desktop-agent-integration-responded' }
>['result'];
type CommandWithoutRequestId =
	DesktopAgentIntegrationManagementRendererCommand extends infer Command
		? Command extends DesktopAgentIntegrationManagementRendererCommand
			? Omit<Command, 'requestId'>
			: never
		: never;

interface PendingRequest {
	readonly integrationId: DesktopAgentIntegrationId | null;
	readonly requestId: DesktopAgentIntegrationManagementRequestId;
	resolve(result: ProtocolResult | null): void;
}

export class DesktopAgentIntegrationManagementClient implements AgentIntegrationManagementControl {
	readonly #createRequestId: () => DesktopAgentIntegrationManagementRequestId;
	#disposed = false;
	#initialization: Promise<AgentIntegrationManagementClientInitialization> | undefined;
	#latestLaunchRevision = 0;
	readonly #listeners = new Set<() => void>();
	#launchedIntegrationIds: readonly DesktopAgentIntegrationId[] = Object.freeze([]);
	#pending: PendingRequest | undefined;
	readonly #port: MessagePort;
	#sequence = 0;
	#snapshot: AgentIntegrationManagementClientSnapshot | undefined;

	constructor(port: MessagePort, createRequestId: () => string = defaultRequestId) {
		this.#port = port;
		this.#createRequestId = () =>
			DesktopAgentIntegrationManagementRequestIdSchema.parse(
				`${createRequestId()}:${++this.#sequence}`,
			);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	initialize(): Promise<AgentIntegrationManagementClientInitialization> {
		this.#initialization ??= this.#initialize();
		return this.#initialization;
	}

	async #initialize(): Promise<AgentIntegrationManagementClientInitialization> {
		if (this.#snapshot !== undefined) return { kind: 'ready' };
		const result = await this.#request({
			type: 'desktop-agent-integration-inventory-requested',
		});
		return result === null ? { kind: 'unavailable' } : { kind: 'ready' };
	}

	getSnapshot = (): AgentIntegrationManagementClientSnapshot => {
		if (this.#snapshot === undefined) {
			throw new Error('Agent integration management must initialize before reading.');
		}
		return this.#snapshot;
	};

	subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	refresh(): Promise<AgentIntegrationManagementClientOperationResult> {
		return this.#operate({ type: 'desktop-agent-integration-refresh-requested' });
	}

	enable(
		integrationId: DesktopAgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult> {
		return this.#operate({
			integrationId,
			type: 'desktop-agent-integration-enable-requested',
		});
	}

	disable(
		integrationId: DesktopAgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult> {
		return this.#operate({
			integrationId,
			type: 'desktop-agent-integration-disable-requested',
		});
	}

	dispose(): void {
		this.#retire(true);
	}

	async #operate(
		command: CommandWithoutRequestId,
	): Promise<AgentIntegrationManagementClientOperationResult> {
		if (this.#snapshot === undefined || this.#disposed) return { kind: 'unavailable' };
		if (this.#pending !== undefined) {
			return {
				code: 'busy',
				integrationId: 'integrationId' in command ? command.integrationId : null,
				kind: 'failed',
				snapshot: protocolSnapshot(this.#snapshot),
			};
		}
		const result = await this.#request(command);
		return result ?? { kind: 'unavailable' };
	}

	#request(command: CommandWithoutRequestId): Promise<ProtocolResult | null> {
		if (this.#disposed || this.#pending !== undefined) return Promise.resolve(null);
		const requestId = this.#createRequestId();
		const message = DesktopAgentIntegrationManagementRendererCommandSchema.parse({
			...command,
			requestId,
		});
		const result = new Promise<ProtocolResult | null>((resolve) => {
			this.#pending = {
				integrationId: 'integrationId' in command ? command.integrationId : null,
				requestId,
				resolve,
			};
		});
		this.#updatePending();
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
		const parsed = DesktopAgentIntegrationManagementMainEventSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		if (parsed.data.type === 'desktop-agent-integration-launch-observed') {
			this.#recordLaunch(
				parsed.data.integrationId,
				parsed.data.revision,
				true,
				parsed.data.integration,
			);
			return;
		}
		const pending = this.#pending;
		if (pending === undefined || parsed.data.requestId !== pending.requestId) {
			this.#retire(true);
			return;
		}
		this.#pending = undefined;
		const result = parsed.data.result;
		for (const integrationId of result.snapshot.launchedIntegrationIds) {
			this.#recordLaunch(integrationId, result.snapshot.revision, false);
		}
		this.#snapshot = {
			...result.snapshot,
			failureCode: result.kind === 'failed' ? result.code : null,
			failureIntegrationId: result.kind === 'failed' ? result.integrationId : null,
			launchedIntegrationIds: [...this.#launchedIntegrationIds],
			pending: false,
			pendingIntegrationId: null,
			revision: Math.max(result.snapshot.revision, this.#latestLaunchRevision),
		};
		this.#emit();
		pending.resolve(result);
	};

	readonly #handleMessageError = (): void => this.#retire(true);

	#recordLaunch(
		integrationId: DesktopAgentIntegrationId,
		revision: number,
		publish = true,
		integration?: DesktopAgentIntegrationSnapshotItem,
	): void {
		const isNew = !this.#launchedIntegrationIds.includes(integrationId);
		if (isNew) {
			this.#launchedIntegrationIds = Object.freeze([
				...this.#launchedIntegrationIds,
				integrationId,
			]);
		}
		this.#latestLaunchRevision = Math.max(this.#latestLaunchRevision, revision);
		if (!publish || this.#snapshot === undefined) return;
		const nextRevision = Math.max(this.#snapshot.revision, this.#latestLaunchRevision);
		const integrationChanged =
			integration !== undefined &&
			this.#snapshot.integrations.some(
				(current) =>
					current.integrationId === integrationId && !integrationItemsEqual(current, integration),
			);
		if (!isNew && !integrationChanged && nextRevision === this.#snapshot.revision) return;
		this.#snapshot = {
			...this.#snapshot,
			integrations:
				integration === undefined
					? this.#snapshot.integrations
					: this.#snapshot.integrations.map((current) =>
							current.integrationId === integrationId ? integration : current,
						),
			launchedIntegrationIds: [...this.#launchedIntegrationIds],
			revision: nextRevision,
		};
		this.#emit();
	}

	#updatePending(): void {
		if (this.#snapshot === undefined) return;
		this.#snapshot = {
			...this.#snapshot,
			failureCode: null,
			failureIntegrationId: null,
			pending: true,
			pendingIntegrationId: this.#pending?.integrationId ?? null,
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
				failureIntegrationId: pending?.integrationId ?? null,
				pending: false,
				pendingIntegrationId: null,
			};
			this.#emit();
		}
		pending?.resolve(null);
	}
}

function integrationItemsEqual(
	left: DesktopAgentIntegrationSnapshotItem,
	right: DesktopAgentIntegrationSnapshotItem,
): boolean {
	return (
		left.availability === right.availability &&
		left.integrationId === right.integrationId &&
		left.issue === right.issue &&
		left.managementMode === right.managementMode &&
		left.state === right.state
	);
}

function protocolSnapshot(
	snapshot: AgentIntegrationManagementClientSnapshot,
): ProtocolResult['snapshot'] {
	return {
		integrations: snapshot.integrations,
		launchedIntegrationIds: [...snapshot.launchedIntegrationIds],
		revision: snapshot.revision,
	};
}

function defaultRequestId(): string {
	return `agent-integration:${createBrowserRandomIdentifier()}`;
}
