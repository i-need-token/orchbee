import {
	ProjectedAttachmentBootstrapSchema,
	type ProjectedAttachmentBootstrap,
} from '@orchbee/workbench-client-protocol';
import {
	AttachTerminalSchema,
	AttachmentAcknowledgedSchema,
	AttachmentCapabilitiesSchema,
	AttachmentIdSchema,
	MessageKind,
	type TerminalIdentity,
} from '@orchbee/terminal-protocol';
import type {
	TerminalPresentationFailure,
	TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';
import type { MessagePortMain } from 'electron';
import type { ProjectedTerminalBrokerMetrics } from '../broker';
import type { HostConnection } from '../host';
import type { DesktopTerminalProjectionTarget } from '../terminal-document-control';

export interface DesktopProjectionBrokerHandle {
	readonly metrics: ProjectedTerminalBrokerMetrics;
	dispose(): Promise<void>;
	start(): void;
}

export interface DesktopTerminalProjectionLease {
	readonly bootstrap: ProjectedAttachmentBootstrap;
	readonly metrics: ProjectedTerminalBrokerMetrics;
	readonly rendererPort: MessagePortMain;
	dispose(): Promise<void>;
}

export interface DesktopTerminalProjectionServiceDependencies {
	readonly createBroker: (
		host: HostConnection,
		port: MessagePortMain,
		bootstrap: ProjectedAttachmentBootstrap,
	) => DesktopProjectionBrokerHandle;
	readonly createMessageChannel: () => {
		readonly port1: MessagePortMain;
		readonly port2: MessagePortMain;
	};
	readonly getProjectionTarget: (
		presentationId: TerminalPresentationId,
	) => DesktopTerminalProjectionTarget | null;
	readonly host: HostConnection;
	readonly randomAttachmentId: () => string;
}

class DesktopTerminalProjectionError extends Error {
	readonly failure: TerminalPresentationFailure;

	constructor(failure: TerminalPresentationFailure) {
		super(failure.code, { cause: failure });
		this.name = 'DesktopTerminalProjectionError';
		this.failure = failure;
	}
}

interface ActiveLease {
	readonly dispose: () => Promise<void>;
	readonly metrics: ProjectedTerminalBrokerMetrics;
	readonly ownsResize: boolean;
}

export class DesktopTerminalProjectionService {
	readonly #activeLeases = new Map<TerminalPresentationId, Set<ActiveLease>>();
	readonly #dependencies: DesktopTerminalProjectionServiceDependencies;
	readonly #openTails = new Map<TerminalPresentationId, Promise<void>>();
	readonly #retirementGenerations = new Map<TerminalPresentationId, number>();
	#disposed = false;

	constructor(dependencies: DesktopTerminalProjectionServiceDependencies) {
		this.#dependencies = dependencies;
	}

	async open(presentationId: TerminalPresentationId): Promise<DesktopTerminalProjectionLease> {
		const previous = this.#openTails.get(presentationId) ?? Promise.resolve();
		const operation = previous.then(async () => this.#openSerialized(presentationId));
		const tail = operation.then(
			() => undefined,
			() => undefined,
		);
		this.#openTails.set(presentationId, tail);
		void tail.then(() => {
			if (this.#openTails.get(presentationId) === tail) {
				this.#openTails.delete(presentationId);
			}
		});
		return operation;
	}

	async #openSerialized(
		presentationId: TerminalPresentationId,
	): Promise<DesktopTerminalProjectionLease> {
		if (this.#disposed) throw projectionError('terminal-projection-unavailable', true);
		const target = this.#dependencies.getProjectionTarget(presentationId);
		if (target === null) throw projectionError('terminal-presentation-not-ready', true);
		const retirementGeneration = this.#retirementGenerations.get(presentationId) ?? 0;
		const attachmentId = AttachmentIdSchema.parse(this.#dependencies.randomAttachmentId());
		const requestResizeLease =
			!target.readOnly &&
			![...(this.#activeLeases.get(presentationId) ?? [])].some((lease) => lease.ownsResize);
		const capabilities = await this.#dependencies.host.requestControl(
			MessageKind.attachTerminal,
			AttachTerminalSchema.parse({
				attachmentId,
				capabilities: { input: !target.readOnly, read: true, resize: requestResizeLease },
				requestResizeLease,
				terminalId: target.identity.terminalId,
			}),
			MessageKind.attachmentCapabilities,
			AttachmentCapabilitiesSchema,
		);
		if (
			!capabilitiesMatch(
				attachmentId,
				target.identity,
				!target.readOnly,
				requestResizeLease,
				capabilities,
			)
		) {
			await this.#dependencies.host
				.requestControl(
					MessageKind.detachTerminal,
					{ attachmentId },
					MessageKind.detachTerminal,
					AttachmentAcknowledgedSchema,
				)
				.catch(() => undefined);
			throw projectionError('terminal-projection-capability-rejected', false);
		}

		const bootstrap = ProjectedAttachmentBootstrapSchema.parse({
			attachmentId,
			canInput: capabilities.capabilities.input,
			canResize: capabilities.resizeLease,
			creationId: target.identity.creationId,
			hostEpoch: target.identity.hostEpoch,
			terminalExit: target.terminalExit,
			terminalId: target.identity.terminalId,
		});
		const channel = this.#dependencies.createMessageChannel();
		const broker = this.#dependencies.createBroker(
			this.#dependencies.host,
			channel.port2,
			bootstrap,
		);
		let disposed = false;
		const activeLease: ActiveLease = {
			dispose: async () => {
				if (disposed) return;
				disposed = true;
				this.#activeLeases.get(presentationId)?.delete(activeLease);
				if (this.#activeLeases.get(presentationId)?.size === 0) {
					this.#activeLeases.delete(presentationId);
				}
				await broker.dispose();
			},
			get metrics() {
				return broker.metrics;
			},
			ownsResize: bootstrap.canResize,
		};
		if (
			this.#disposed ||
			(this.#retirementGenerations.get(presentationId) ?? 0) !== retirementGeneration
		) {
			await activeLease.dispose();
			throw projectionError('terminal-projection-retired', false);
		}
		const leases = this.#activeLeases.get(presentationId) ?? new Set<ActiveLease>();
		leases.add(activeLease);
		this.#activeLeases.set(presentationId, leases);
		broker.start();
		return {
			bootstrap,
			dispose: activeLease.dispose,
			get metrics() {
				return broker.metrics;
			},
			rendererPort: channel.port1,
		};
	}

	getMetrics(): readonly ProjectedTerminalBrokerMetrics[] {
		return [...this.#activeLeases.values()].flatMap((leases) =>
			[...leases].map((lease) => lease.metrics),
		);
	}

	async retirePresentation(presentationId: TerminalPresentationId): Promise<void> {
		this.#retirementGenerations.set(
			presentationId,
			(this.#retirementGenerations.get(presentationId) ?? 0) + 1,
		);
		await this.#openTails.get(presentationId);
		const leases = [...(this.#activeLeases.get(presentationId) ?? [])];
		await Promise.all(leases.map(async (lease) => lease.dispose()));
	}

	async dispose(): Promise<void> {
		if (this.#disposed) return;
		this.#disposed = true;
		await Promise.all(this.#openTails.values());
		const leases = [...this.#activeLeases.values()].flatMap((active) => [...active]);
		await Promise.all(leases.map(async (lease) => lease.dispose()));
	}
}

function capabilitiesMatch(
	attachmentId: ReturnType<typeof AttachmentIdSchema.parse>,
	identity: TerminalIdentity,
	canInput: boolean,
	requestedResizeLease: boolean,
	capabilities: ReturnType<typeof AttachmentCapabilitiesSchema.parse>,
): boolean {
	return (
		capabilities.attachmentId === attachmentId &&
		capabilities.terminalId === identity.terminalId &&
		capabilities.capabilities.read &&
		capabilities.capabilities.input === canInput &&
		capabilities.capabilities.resize === requestedResizeLease &&
		capabilities.resizeLease === requestedResizeLease
	);
}

function projectionError(code: string, retryable: boolean): DesktopTerminalProjectionError {
	return new DesktopTerminalProjectionError({ code, retryable });
}
