import type { TerminalGeometry } from '@orchbee/terminal-protocol';
import type { WorkbenchSharedDocument } from '@orchbee/workbench/document';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';

import {
	resolveDesktopTerminalPresentationEffectTarget,
	type DesktopTerminalEffectTarget,
} from '../client-session';
import { EnvironmentIdSchema, type EnvironmentId } from '../environment';
import type {
	DesktopTerminalProjectionLease,
	DesktopTerminalProjectionOpenOptions,
} from './DesktopTerminalProjectionService';

interface DesktopTerminalProjectionRuntimePort {
	open(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
		options?: DesktopTerminalProjectionOpenOptions,
	): Promise<DesktopTerminalProjectionLease>;
	openReadOnly(presentationId: TerminalPresentationId): Promise<DesktopTerminalProjectionLease>;
}

interface DesktopSshEnvironmentTerminalProjectionRuntimePort {
	open(
		environmentId: EnvironmentId,
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
		options?: DesktopTerminalProjectionOpenOptions,
	): Promise<DesktopTerminalProjectionLease>;
	openReadOnly(
		environmentId: EnvironmentId,
		presentationId: TerminalPresentationId,
	): Promise<DesktopTerminalProjectionLease>;
}

interface DesktopTerminalProjectionRouterOptions {
	readonly getDocument: () => WorkbenchSharedDocument | null;
	readonly localRuntime: DesktopTerminalProjectionRuntimePort;
	readonly remoteRuntimes: DesktopSshEnvironmentTerminalProjectionRuntimePort;
}

export class DesktopTerminalProjectionRouter {
	readonly #getDocument: DesktopTerminalProjectionRouterOptions['getDocument'];
	readonly #localRuntime: DesktopTerminalProjectionRuntimePort;
	readonly #publicationTargets = new WeakMap<
		DesktopTerminalProjectionLease,
		DesktopTerminalEffectTarget
	>();
	readonly #remoteRuntimes: DesktopSshEnvironmentTerminalProjectionRuntimePort;

	constructor(options: DesktopTerminalProjectionRouterOptions) {
		this.#getDocument = options.getDocument;
		this.#localRuntime = options.localRuntime;
		this.#remoteRuntimes = options.remoteRuntimes;
	}

	async open(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
		options: DesktopTerminalProjectionOpenOptions = { brokerEffectTarget: 'desktop' },
	): Promise<DesktopTerminalProjectionLease> {
		const target = this.#resolveTarget(presentationId);
		const lease =
			target.kind === 'local'
				? await this.#localRuntime.open(presentationId, geometry, options)
				: await this.#remoteRuntimes.open(
						EnvironmentIdSchema.parse(target.environmentId),
						presentationId,
						geometry,
						options,
					);
		if (this.#targetStillCurrent(presentationId, target)) {
			this.#publicationTargets.set(lease, target);
			return lease;
		}
		try {
			await lease.dispose();
		} catch {
			throw projectionFailure('terminal-projection-cleanup-failed', true);
		}
		throw projectionFailure('terminal-projection-retired', false);
	}

	async openReadOnly(
		presentationId: TerminalPresentationId,
	): Promise<DesktopTerminalProjectionLease> {
		const target = this.#resolveTarget(presentationId);
		const lease =
			target.kind === 'local'
				? await this.#localRuntime.openReadOnly(presentationId)
				: await this.#remoteRuntimes.openReadOnly(
						EnvironmentIdSchema.parse(target.environmentId),
						presentationId,
					);
		if (this.#targetStillCurrent(presentationId, target)) {
			this.#publicationTargets.set(lease, target);
			return lease;
		}
		try {
			await lease.dispose();
		} catch {
			throw projectionFailure('terminal-projection-cleanup-failed', true);
		}
		throw projectionFailure('terminal-projection-retired', false);
	}

	isPublicationTargetCurrent(
		presentationId: TerminalPresentationId,
		lease: DesktopTerminalProjectionLease,
	): boolean {
		const target = this.#publicationTargets.get(lease);
		this.#publicationTargets.delete(lease);
		return target !== undefined && this.#targetStillCurrent(presentationId, target);
	}

	#resolveTarget(presentationId: TerminalPresentationId): DesktopTerminalEffectTarget {
		const document = this.#getDocument();
		if (document === null) {
			throw projectionFailure('terminal-presentation-not-ready', true);
		}
		try {
			return resolveDesktopTerminalPresentationEffectTarget(document, presentationId);
		} catch {
			throw projectionFailure('terminal-presentation-not-ready', true);
		}
	}

	#targetStillCurrent(
		presentationId: TerminalPresentationId,
		target: DesktopTerminalEffectTarget,
	): boolean {
		try {
			const current = this.#resolveTarget(presentationId);
			return targetsEqual(current, target);
		} catch {
			return false;
		}
	}
}

function targetsEqual(
	left: DesktopTerminalEffectTarget,
	right: DesktopTerminalEffectTarget,
): boolean {
	return (
		left.kind === right.kind &&
		(left.kind === 'local' ||
			(right.kind === 'ssh-environment' && left.environmentId === right.environmentId))
	);
}

function projectionFailure(code: string, retryable: boolean): Error {
	return new Error(code, { cause: { code, retryable } });
}
