import type { ProjectedPasteResult } from '@orchbee/terminal';

import type { ProjectedTerminalRendererSession } from '../../projected-terminal-renderer';
import type { TerminalProjection, TerminalProjectionRequest } from '../../terminal-projection';

const AUTOMATIC_RETRY_DELAYS_MILLISECONDS = [250, 500, 1_000] as const;

export type ProjectedTerminalRecoveryState =
	| { readonly kind: 'connecting' }
	| { readonly attempt: number; readonly kind: 'recovering' }
	| { readonly kind: 'ready' }
	| { readonly kind: 'failed' };

interface ManagedProjectionRequest {
	release(): void;
	readonly request: TerminalProjectionRequest;
}

interface ManagedProjectedTerminalSession extends ManagedProjectionRequest {
	readonly session: ProjectedTerminalRendererSession;
}

export interface ProjectedTerminalRecoveryControllerOptions {
	readonly onStateChange: (state: ProjectedTerminalRecoveryState) => void;
	readonly open: () => TerminalProjectionRequest;
	readonly startRenderer: (
		projection: TerminalProjection,
	) => Promise<ProjectedTerminalRendererSession>;
}

export type ProjectedTerminalRecoveryFileDropResult =
	| ProjectedPasteResult
	| { readonly kind: 'unavailable' };

type RecoveryAttemptSource = 'automatic' | 'initial' | 'manual';

/** Owns one bounded projected-attachment recovery sequence for a mounted terminal surface. */
export class ProjectedTerminalRecoveryController {
	readonly #onStateChange: ProjectedTerminalRecoveryControllerOptions['onStateChange'];
	readonly #open: ProjectedTerminalRecoveryControllerOptions['open'];
	readonly #startRenderer: ProjectedTerminalRecoveryControllerOptions['startRenderer'];
	#active: ManagedProjectedTerminalSession | undefined;
	#automaticRetryIndex = 0;
	#candidate: ManagedProjectedTerminalSession | undefined;
	#disposed = false;
	readonly #disposedPromise: Promise<void>;
	#frozen: ManagedProjectedTerminalSession | undefined;
	#pending: ManagedProjectionRequest | undefined;
	#resolveDisposed: (() => void) | undefined;
	#retryTimer: ReturnType<typeof globalThis.setTimeout> | undefined;

	constructor(options: ProjectedTerminalRecoveryControllerOptions) {
		this.#onStateChange = options.onStateChange;
		this.#open = options.open;
		this.#startRenderer = options.startRenderer;
		this.#disposedPromise = new Promise<void>((resolve) => {
			this.#resolveDisposed = resolve;
		});
	}

	start(): void {
		if (this.#disposed || this.#active !== undefined || this.#pending !== undefined) return;
		this.#onStateChange({ kind: 'connecting' });
		this.#beginAttempt('initial');
	}

	retry(): void {
		if (
			this.#disposed ||
			this.#active !== undefined ||
			this.#pending !== undefined ||
			this.#retryTimer !== undefined
		) {
			return;
		}
		this.#onStateChange({ attempt: 1, kind: 'recovering' });
		this.#beginAttempt('manual');
	}

	async pasteFileDrop(text: string): Promise<ProjectedTerminalRecoveryFileDropResult> {
		if (this.#disposed || this.#active === undefined) return { kind: 'unavailable' };
		return await this.#active.session.pasteFileDrop(text);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#resolveDisposed?.();
		this.#resolveDisposed = undefined;
		if (this.#retryTimer !== undefined) globalThis.clearTimeout(this.#retryTimer);
		this.#retryTimer = undefined;
		this.#pending?.release();
		this.#pending = undefined;
		this.#candidate?.session.dispose();
		this.#candidate?.release();
		this.#candidate = undefined;
		this.#active?.session.dispose();
		this.#active?.release();
		this.#active = undefined;
		this.#frozen?.session.dispose();
		this.#frozen?.release();
		this.#frozen = undefined;
	}

	#beginAttempt(source: RecoveryAttemptSource): void {
		if (this.#disposed || this.#pending !== undefined) return;
		let request: TerminalProjectionRequest;
		try {
			request = this.#open();
		} catch {
			this.#handleAttemptFailure(source);
			return;
		}
		if (this.#disposed) {
			request.release();
			return;
		}
		const pending = managedRequest(request);
		this.#pending = pending;
		void this.#resolveAttempt(pending, source);
	}

	async #resolveAttempt(
		pending: ManagedProjectionRequest,
		source: RecoveryAttemptSource,
	): Promise<void> {
		let projection: TerminalProjection;
		try {
			projection = await pending.request.result;
		} catch {
			if (!this.#ownsPending(pending)) return;
			this.#pending = undefined;
			pending.release();
			this.#handleAttemptFailure(source);
			return;
		}
		if (!this.#ownsPending(pending)) {
			projection.transport.close();
			pending.release();
			return;
		}

		let session: ProjectedTerminalRendererSession;
		try {
			session = await this.#startRenderer(projection);
		} catch {
			if (!this.#ownsPending(pending)) return;
			this.#pending = undefined;
			pending.release();
			this.#handleAttemptFailure(source);
			return;
		}
		if (!this.#ownsPending(pending)) {
			session.dispose();
			pending.release();
			return;
		}

		const managed: ManagedProjectedTerminalSession = { ...pending, session };
		this.#candidate = managed;
		const outcome = session.isTransportAvailable()
			? await Promise.race([
					session.presentationReady.then(() => 'ready' as const),
					session.transportUnavailable.then(() => 'unavailable' as const),
					this.#disposedPromise.then(() => 'disposed' as const),
				])
			: 'unavailable';
		if (!this.#ownsPending(pending) || this.#candidate !== managed) {
			if (this.#candidate === managed) session.dispose();
			pending.release();
			return;
		}
		this.#candidate = undefined;
		this.#pending = undefined;
		if (outcome !== 'ready' || !session.isTransportAvailable()) {
			session.dispose();
			pending.release();
			this.#handleAttemptFailure(source);
			return;
		}

		this.#frozen?.session.dispose();
		this.#frozen?.release();
		this.#frozen = undefined;
		this.#active = managed;
		this.#automaticRetryIndex = 0;
		this.#onStateChange({ kind: 'ready' });
		void session.transportUnavailable.then(() => this.#handleTransportUnavailable(managed));
	}

	#handleTransportUnavailable(managed: ManagedProjectedTerminalSession): void {
		if (this.#disposed || this.#active !== managed) return;
		this.#active = undefined;
		managed.session.freeze();
		managed.release();
		this.#frozen?.session.dispose();
		this.#frozen?.release();
		this.#frozen = managed;
		this.#automaticRetryIndex = 0;
		this.#scheduleAutomaticRetry();
	}

	#handleAttemptFailure(source: RecoveryAttemptSource): void {
		if (this.#disposed) return;
		if (source === 'manual') {
			this.#onStateChange({ kind: 'failed' });
			return;
		}
		this.#scheduleAutomaticRetry();
	}

	#scheduleAutomaticRetry(): void {
		const delay = AUTOMATIC_RETRY_DELAYS_MILLISECONDS[this.#automaticRetryIndex];
		if (delay === undefined) {
			this.#onStateChange({ kind: 'failed' });
			return;
		}
		this.#automaticRetryIndex += 1;
		this.#onStateChange({ attempt: this.#automaticRetryIndex, kind: 'recovering' });
		this.#retryTimer = globalThis.setTimeout(() => {
			this.#retryTimer = undefined;
			this.#beginAttempt('automatic');
		}, delay);
	}

	#ownsPending(pending: ManagedProjectionRequest): boolean {
		return !this.#disposed && this.#pending === pending;
	}
}

function managedRequest(request: TerminalProjectionRequest): ManagedProjectionRequest {
	let released = false;
	return {
		release: () => {
			if (released) return;
			released = true;
			request.release();
		},
		request,
	};
}
