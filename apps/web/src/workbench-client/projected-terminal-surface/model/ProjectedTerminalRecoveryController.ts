import type { ProjectedTerminalPresentationSnapshot } from '@orchbee/terminal';

import type { ProjectedTerminalControlPresentation } from '../../projected-terminal-actions';
import type {
	MobileTerminalInputState,
	MobileTerminalKey,
	MobileTerminalModifier,
	ProjectedTerminalOutcome,
	ProjectedTerminalRendererSession,
} from '../../projected-terminal-renderer';
import type { TerminalProjection, TerminalProjectionRequest } from '../../terminal-projection';
import { runApplicationCleanup } from '../../../runApplicationCleanup';

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
	readonly onActivationReady?: (() => void) | undefined;
	readonly onDisplayReady?: (() => void) | undefined;
	readonly onRetainedSnapshot?:
		| ((snapshot: ProjectedTerminalPresentationSnapshot) => void)
		| undefined;
	readonly onStateChange: (state: ProjectedTerminalRecoveryState) => void;
	readonly open: () => TerminalProjectionRequest;
	readonly startRenderer: (
		projection: TerminalProjection,
	) => Promise<ProjectedTerminalRendererSession>;
}

type RecoveryAttemptSource = 'automatic' | 'initial' | 'manual';

/** Owns one bounded projected-attachment recovery sequence for a mounted terminal surface. */
export class ProjectedTerminalRecoveryController {
	readonly #onActivationReady: () => void;
	readonly #onDisplayReady: () => void;
	readonly #onRetainedSnapshot: (snapshot: ProjectedTerminalPresentationSnapshot) => void;
	readonly #onStateChange: ProjectedTerminalRecoveryControllerOptions['onStateChange'];
	readonly #open: ProjectedTerminalRecoveryControllerOptions['open'];
	readonly #startRenderer: ProjectedTerminalRecoveryControllerOptions['startRenderer'];
	#active: ManagedProjectedTerminalSession | undefined;
	#activeControlSubscription: { dispose(): void } | undefined;
	#activeMobileInputSubscription: { dispose(): void } | undefined;
	#automaticRetryIndex = 0;
	readonly #controlPresentationListeners = new Set<() => void>();
	readonly #mobileInputStateListeners = new Set<() => void>();
	#candidate: ManagedProjectedTerminalSession | undefined;
	#disposed = false;
	readonly #disposedPromise: Promise<void>;
	#frozen: ManagedProjectedTerminalSession | undefined;
	#opening: symbol | undefined;
	#pending: ManagedProjectionRequest | undefined;
	#resolveDisposed: (() => void) | undefined;
	#retained = false;
	#retryTimer: ReturnType<typeof globalThis.setTimeout> | undefined;

	constructor(options: ProjectedTerminalRecoveryControllerOptions) {
		this.#onActivationReady = options.onActivationReady ?? (() => undefined);
		this.#onDisplayReady = options.onDisplayReady ?? (() => undefined);
		this.#onRetainedSnapshot = options.onRetainedSnapshot ?? (() => undefined);
		this.#onStateChange = options.onStateChange;
		this.#open = options.open;
		this.#startRenderer = options.startRenderer;
		this.#disposedPromise = new Promise<void>((resolve) => {
			this.#resolveDisposed = resolve;
		});
	}

	start(): void {
		if (
			this.#disposed ||
			this.#active !== undefined ||
			this.#opening !== undefined ||
			this.#pending !== undefined
		)
			return;
		this.#retained = false;
		this.#beginAttempt('initial');
	}

	/** Retire attachment and input ownership while keeping the last painted DOM in place. */
	retain(): void {
		if (this.#disposed || this.#retained) return;
		this.#retained = true;
		this.#opening = undefined;
		if (this.#retryTimer !== undefined) globalThis.clearTimeout(this.#retryTimer);
		this.#retryTimer = undefined;
		const candidate = this.#candidate;
		if (candidate !== undefined) {
			this.#candidate = undefined;
			this.#pending = undefined;
			this.#freezeProjection(candidate, 'retention');
		} else if (this.#active !== undefined) {
			const active = this.#active;
			this.#active = undefined;
			this.#freezeProjection(active, 'retention');
			if (this.#disposed) return;
			this.#notifyMobileInputState();
			this.#notifyControlPresentation();
		} else {
			const pending = this.#pending;
			this.#pending = undefined;
			pending?.release();
			if (this.#disposed || !this.#retained) return;
			const snapshot = this.#frozen?.session.capturePresentationSnapshot?.() ?? null;
			if (!this.#disposed && this.#retained && snapshot !== null)
				this.#onRetainedSnapshot(snapshot);
		}
	}

	/** Acquire a fresh attachment behind an existing frozen presentation. */
	resume(): void {
		if (this.#disposed || !this.#retained) return;
		this.#retained = false;
		if (this.#frozen !== undefined) this.#onDisplayReady();
		this.#beginAttempt('initial');
	}

	retry(): void {
		if (
			this.#disposed ||
			this.#retained ||
			this.#active !== undefined ||
			this.#opening !== undefined ||
			this.#pending !== undefined ||
			this.#retryTimer !== undefined
		) {
			return;
		}
		this.#beginAttempt('manual');
	}

	focusTerminal(): boolean {
		if (this.#disposed || this.#active === undefined) return false;
		this.#active.session.focus();
		return true;
	}

	getControlPresentation(): ProjectedTerminalControlPresentation {
		return this.#disposed ? 'none' : (this.#active?.session.getControlPresentation?.() ?? 'none');
	}

	isLiveCropped(): boolean {
		return !this.#disposed && this.#active?.session.isLiveCropped?.() === true;
	}

	getTerminalOutcome(): ProjectedTerminalOutcome | undefined {
		return this.#disposed ? undefined : this.#active?.session.getTerminalOutcome?.();
	}

	subscribeControlPresentation(listener: () => void): { dispose(): void } {
		if (!this.#disposed) this.#controlPresentationListeners.add(listener);
		return { dispose: () => this.#controlPresentationListeners.delete(listener) };
	}

	async takeControl(): Promise<boolean> {
		const active = this.#active;
		if (this.#disposed || active === undefined) return false;
		return (await active.session.takeControl?.()) ?? false;
	}

	getMobileInputState(): MobileTerminalInputState {
		return this.#disposed
			? unavailableMobileInputState()
			: (this.#active?.session.getMobileInputState?.() ?? unavailableMobileInputState());
	}

	sendMobileKey(key: MobileTerminalKey): boolean {
		return this.#disposed ? false : (this.#active?.session.sendMobileKey?.(key) ?? false);
	}

	toggleMobileKeyboard(): boolean {
		return this.#disposed ? false : (this.#active?.session.toggleMobileKeyboard?.() ?? false);
	}

	toggleMobileModifier(modifier: MobileTerminalModifier): void {
		if (!this.#disposed) this.#active?.session.toggleMobileModifier?.(modifier);
	}

	subscribeMobileInputState(listener: () => void): { dispose(): void } {
		if (!this.#disposed) this.#mobileInputStateListeners.add(listener);
		return { dispose: () => this.#mobileInputStateListeners.delete(listener) };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#opening = undefined;
		const controlSubscription = this.#activeControlSubscription;
		const mobileInputSubscription = this.#activeMobileInputSubscription;
		const controlListeners = [...this.#controlPresentationListeners];
		const mobileListeners = [...this.#mobileInputStateListeners];
		const resolveDisposed = this.#resolveDisposed;
		const retryTimer = this.#retryTimer;
		const pending = this.#pending;
		const candidate = this.#candidate;
		const active = this.#active;
		const frozen = this.#frozen;
		this.#activeControlSubscription = undefined;
		this.#activeMobileInputSubscription = undefined;
		this.#resolveDisposed = undefined;
		this.#retryTimer = undefined;
		this.#pending = undefined;
		this.#candidate = undefined;
		this.#active = undefined;
		this.#frozen = undefined;
		runApplicationCleanup(
			[
				() => {
					if (retryTimer !== undefined) globalThis.clearTimeout(retryTimer);
				},
				() => resolveDisposed?.(),
				() => controlSubscription?.dispose(),
				() => mobileInputSubscription?.dispose(),
				...controlListeners.map((listener) => () => {
					if (this.#controlPresentationListeners.has(listener)) listener();
				}),
				...mobileListeners.map((listener) => () => {
					if (this.#mobileInputStateListeners.has(listener)) listener();
				}),
				() => this.#controlPresentationListeners.clear(),
				() => this.#mobileInputStateListeners.clear(),
				() => pending?.release(),
				() => disposeManagedSession(candidate),
				() => disposeManagedSession(active),
				() => disposeManagedSession(frozen),
			],
			'Projected terminal cleanup failed',
		);
	}

	#beginAttempt(source: RecoveryAttemptSource): void {
		if (
			this.#disposed ||
			this.#retained ||
			this.#active !== undefined ||
			this.#opening !== undefined ||
			this.#pending !== undefined
		)
			return;
		const opening = Symbol('projection-attempt');
		this.#opening = opening;
		try {
			if (source === 'initial') this.#onStateChange({ kind: 'connecting' });
			else if (source === 'manual') this.#onStateChange({ attempt: 1, kind: 'recovering' });
			if (this.#opening !== opening) return;
			let request: TerminalProjectionRequest;
			try {
				request = this.#open();
			} catch {
				if (this.#opening !== opening) return;
				this.#opening = undefined;
				this.#handleAttemptFailure(source);
				return;
			}
			const pending = managedRequest(request);
			if (this.#opening !== opening) {
				void this.#resolveAttempt(pending, source);
				pending.release();
				return;
			}
			this.#opening = undefined;
			this.#pending = pending;
			void this.#resolveAttempt(pending, source);
		} finally {
			if (this.#opening === opening) this.#opening = undefined;
		}
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
		let candidateDisplayReady = false;
		void (session.displayReady ?? session.presentationReady).then(() => {
			candidateDisplayReady = true;
			if (this.#ownsPending(pending) && this.#candidate === managed) this.#onDisplayReady();
		});
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
		if (outcome !== 'ready' || !session.isTransportAvailable()) {
			this.#candidate = undefined;
			this.#pending = undefined;
			if (candidateDisplayReady && this.#frozen === undefined) {
				this.#freezeProjection(managed, 'recovery');
			} else {
				session.dispose();
				pending.release();
			}
			this.#handleAttemptFailure(source);
			return;
		}

		const frozen = this.#frozen;
		this.#frozen = undefined;
		disposeManagedSession(frozen);
		if (!this.#ownsPending(pending) || this.#candidate !== managed) return;
		this.#candidate = undefined;
		this.#pending = undefined;
		this.#active = managed;
		this.#bindActiveMobileInput(managed);
		if (!this.#ownsActive(managed)) return;
		this.#bindActiveControl(managed);
		if (!this.#ownsActive(managed)) return;
		this.#automaticRetryIndex = 0;
		this.#onStateChange({ kind: 'ready' });
		if (!this.#ownsActive(managed)) return;
		this.#onDisplayReady();
		if (!this.#ownsActive(managed)) return;
		this.#onActivationReady();
		if (!this.#ownsActive(managed)) return;
		void session.transportUnavailable.then(() => this.#handleTransportUnavailable(managed));
	}

	#handleTransportUnavailable(managed: ManagedProjectedTerminalSession): void {
		if (!this.#ownsActive(managed)) return;
		this.#active = undefined;
		this.#automaticRetryIndex = 0;
		this.#freezeProjection(managed, 'recovery');
		if (!this.#ownsFrozen(managed)) return;
		this.#notifyMobileInputState();
		this.#notifyControlPresentation();
		this.#scheduleAutomaticRetry();
	}

	#handleAttemptFailure(source: RecoveryAttemptSource): void {
		if (
			this.#disposed ||
			this.#retained ||
			this.#pending !== undefined ||
			this.#opening !== undefined ||
			this.#active !== undefined
		)
			return;
		if (source === 'manual') {
			this.#onStateChange({ kind: 'failed' });
			return;
		}
		this.#scheduleAutomaticRetry();
	}

	#scheduleAutomaticRetry(): void {
		if (
			this.#disposed ||
			this.#retained ||
			this.#pending !== undefined ||
			this.#opening !== undefined ||
			this.#active !== undefined ||
			this.#retryTimer !== undefined
		)
			return;
		const delay = AUTOMATIC_RETRY_DELAYS_MILLISECONDS[this.#automaticRetryIndex];
		if (delay === undefined) {
			this.#onStateChange({ kind: 'failed' });
			return;
		}
		this.#automaticRetryIndex += 1;
		const timer = globalThis.setTimeout(() => {
			if (this.#retryTimer !== timer) return;
			this.#retryTimer = undefined;
			this.#beginAttempt('automatic');
		}, delay);
		this.#retryTimer = timer;
		this.#onStateChange({ attempt: this.#automaticRetryIndex, kind: 'recovering' });
	}

	#ownsPending(pending: ManagedProjectionRequest): boolean {
		return !this.#disposed && !this.#retained && this.#pending === pending;
	}

	#ownsActive(managed: ManagedProjectedTerminalSession): boolean {
		return !this.#disposed && !this.#retained && this.#active === managed;
	}

	#ownsFrozen(managed: ManagedProjectedTerminalSession): boolean {
		return !this.#disposed && this.#frozen === managed;
	}

	#freezeProjection(
		managed: ManagedProjectedTerminalSession,
		source: 'retention' | 'recovery',
	): void {
		let previous = this.#frozen;
		this.#frozen = managed;
		try {
			this.#unbindActiveMobileInput();
			if (!this.#ownsFrozen(managed)) return;
			this.#unbindActiveControl();
			if (!this.#ownsFrozen(managed)) return;
			const snapshot =
				source === 'retention' ? (managed.session.capturePresentationSnapshot?.() ?? null) : null;
			if (!this.#ownsFrozen(managed)) return;
			if (snapshot !== null) this.#onRetainedSnapshot(snapshot);
			if (!this.#ownsFrozen(managed)) return;
			managed.session.freeze();
			if (!this.#ownsFrozen(managed)) return;
			managed.release();
			if (!this.#ownsFrozen(managed)) return;
			if (source === 'retention' && snapshot === null && previous !== undefined) {
				const retainedSnapshot = previous.session.capturePresentationSnapshot?.() ?? null;
				if (!this.#ownsFrozen(managed)) return;
				if (retainedSnapshot !== null) this.#onRetainedSnapshot(retainedSnapshot);
				if (!this.#ownsFrozen(managed)) return;
				this.#frozen = previous;
				previous = undefined;
				disposeManagedSession(managed);
			}
		} finally {
			disposeManagedSession(previous);
		}
	}

	#bindActiveMobileInput(managed: ManagedProjectedTerminalSession): void {
		this.#unbindActiveMobileInput();
		if (!this.#ownsActive(managed)) return;
		const subscription = managed.session.subscribeMobileInputState?.(() =>
			this.#notifyMobileInputState(),
		);
		if (!this.#ownsActive(managed)) {
			subscription?.dispose();
			return;
		}
		this.#activeMobileInputSubscription = subscription;
		this.#notifyMobileInputState();
	}

	#unbindActiveMobileInput(): void {
		const subscription = this.#activeMobileInputSubscription;
		this.#activeMobileInputSubscription = undefined;
		subscription?.dispose();
	}

	#bindActiveControl(managed: ManagedProjectedTerminalSession): void {
		this.#unbindActiveControl();
		if (!this.#ownsActive(managed)) return;
		const subscription = managed.session.subscribeControlPresentation?.(() =>
			this.#notifyControlPresentation(),
		);
		if (!this.#ownsActive(managed)) {
			subscription?.dispose();
			return;
		}
		this.#activeControlSubscription = subscription;
		this.#notifyControlPresentation();
	}

	#unbindActiveControl(): void {
		const subscription = this.#activeControlSubscription;
		this.#activeControlSubscription = undefined;
		subscription?.dispose();
	}

	#notifyControlPresentation(): void {
		for (const listener of this.#controlPresentationListeners) listener();
	}

	#notifyMobileInputState(): void {
		for (const listener of this.#mobileInputStateListeners) listener();
	}
}

function disposeManagedSession(managed: ManagedProjectedTerminalSession | undefined): void {
	if (managed === undefined) return;
	runApplicationCleanup(
		[() => managed.session.dispose(), () => managed.release()],
		'Projected terminal cleanup failed',
	);
}

function unavailableMobileInputState(): MobileTerminalInputState {
	return {
		alt: false,
		control: false,
		focused: false,
		readiness: { kind: 'unavailable' },
	};
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
