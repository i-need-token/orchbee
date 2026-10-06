import type { TerminalGeometry } from '@orchbee/terminal-protocol';
import type { WorkbenchSharedDocument } from '@orchbee/workbench/document';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';

import {
	resolveDesktopTerminalPresentationEffectTarget,
	type DesktopTerminalEffectTarget,
} from '../client-session';

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

interface DesktopTerminalProjectionRouterOptions {
	readonly getDocument: () => WorkbenchSharedDocument | null;
	readonly localRuntime: DesktopTerminalProjectionRuntimePort;
}

export class DesktopTerminalProjectionRouter {
	readonly #getDocument: DesktopTerminalProjectionRouterOptions['getDocument'];
	readonly #localRuntime: DesktopTerminalProjectionRuntimePort;
	readonly #publicationTargets = new WeakMap<
		DesktopTerminalProjectionLease,
		DesktopTerminalEffectTarget
	>();

	constructor(options: DesktopTerminalProjectionRouterOptions) {
		this.#getDocument = options.getDocument;
		this.#localRuntime = options.localRuntime;
	}

	async open(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
		options: DesktopTerminalProjectionOpenOptions = { brokerEffectTarget: 'desktop' },
	): Promise<DesktopTerminalProjectionLease> {
		const target = this.#resolveTarget(presentationId);
		const lease = await this.#localRuntime.open(presentationId, geometry, options);
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
		const lease = await this.#localRuntime.openReadOnly(presentationId);
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
	return left.kind === right.kind;
}

function projectionFailure(code: string, retryable: boolean): Error {
	return new Error(code, { cause: { code, retryable } });
}
