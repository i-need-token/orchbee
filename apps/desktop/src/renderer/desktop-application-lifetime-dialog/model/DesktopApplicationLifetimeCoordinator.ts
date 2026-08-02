import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalSessionStopSnapshot,
} from '@orchbee/workbench';
import type { WorkbenchTopologyRevision } from '@orchbee/workbench/topology';

import type {
	DesktopApplicationExitChoice,
	DesktopApplicationExitFailure,
	DesktopApplicationExitProgress,
	DesktopApplicationExitRequestId,
	DesktopApplicationLifetimeMainEvent,
	DesktopApplicationLifetimeRendererCommand,
} from '../../../shared';
import type { DesktopWorkbenchTopologyCoordinator } from '../../workbench-topology-control';

type Listener = () => void;

interface LifetimePort {
	post(command: DesktopApplicationLifetimeRendererCommand): boolean;
	subscribe(listener: (event: DesktopApplicationLifetimeMainEvent) => void): () => void;
}

type LifetimeRuntime = Pick<
	WorkbenchProjectWorkspaceRuntime,
	| 'getTerminalSessionStopSnapshot'
	| 'requestAllTerminalSessionsStop'
	| 'retryFailedTerminalSessionStops'
	| 'subscribeTopology'
>;

type LifetimeTopology = Pick<DesktopWorkbenchTopologyCoordinator, 'committedRevision' | 'flush'>;

export type DesktopApplicationLifetimeState =
	| { readonly kind: 'idle' }
	| {
			readonly kind: 'awaiting-choice';
			readonly requestId: DesktopApplicationExitRequestId;
			readonly sessionCount: number;
	  }
	| {
			readonly kind: 'working';
			readonly progress: DesktopApplicationExitProgress;
			readonly requestId: DesktopApplicationExitRequestId;
	  }
	| {
			readonly failures: readonly DesktopApplicationExitFailure[];
			readonly kind: 'failed';
			readonly requestId: DesktopApplicationExitRequestId;
			readonly retry: 'main' | 'terminal' | null;
	  }
	| { readonly kind: 'exiting'; readonly requestId: DesktopApplicationExitRequestId };

export class DesktopApplicationLifetimeCoordinator {
	#disposed = false;
	readonly #lifetimePort: LifetimePort;
	readonly #listeners = new Set<Listener>();
	#preparingStop = false;
	readonly #runtime: LifetimeRuntime;
	#state: DesktopApplicationLifetimeState = { kind: 'idle' };
	#stopDurableRevision: WorkbenchTopologyRevision | null = null;
	readonly #topology: LifetimeTopology;
	readonly #unsubscribeLifetime: () => void;
	readonly #unsubscribeTopology: () => void;

	constructor(runtime: LifetimeRuntime, topology: LifetimeTopology, lifetimePort: LifetimePort) {
		this.#runtime = runtime;
		this.#topology = topology;
		this.#lifetimePort = lifetimePort;
		this.#unsubscribeLifetime = lifetimePort.subscribe(this.#handleMainEvent);
		this.#unsubscribeTopology = runtime.subscribeTopology(this.#observeStop);
	}

	getSnapshot = (): DesktopApplicationLifetimeState => this.#state;

	subscribe = (listener: Listener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	choose(choice: DesktopApplicationExitChoice): void {
		if (this.#state.kind !== 'awaiting-choice' || this.#disposed) return;
		const { requestId } = this.#state;
		this.#post({ choice, requestId, type: 'desktop-application-exit-choice' });
		if (choice === 'cancel') {
			this.#post({ requestId, type: 'desktop-application-exit-cancelled' });
			this.#setState({ kind: 'idle' });
			return;
		}
		if (choice === 'preserve') {
			void this.#preparePreserve(requestId, false);
			return;
		}
		void this.#beginStop(requestId);
	}

	retry(): void {
		if (this.#state.kind !== 'failed' || this.#disposed) return;
		const { requestId, retry } = this.#state;
		if (retry === 'main') {
			this.#setWorking(requestId, 'draining-unreferenced', 0, 0, 0);
			this.#post({ requestId, type: 'desktop-application-exit-main-retry-requested' });
			return;
		}
		if (retry !== 'terminal') return;
		const result = this.#runtime.retryFailedTerminalSessionStops();
		if (result.kind === 'rejected') {
			this.#failRenderer(requestId, [{ code: 'terminal-stop-failed', count: 1, retryable: false }]);
			return;
		}
		void this.#flushStopWriteAhead(requestId);
	}

	quitAnyway(): void {
		if (this.#state.kind !== 'failed' || this.#disposed) return;
		const { requestId } = this.#state;
		this.#post({ requestId, type: 'desktop-application-exit-quit-anyway-requested' });
		this.#setState({ kind: 'exiting', requestId });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#unsubscribeLifetime();
		this.#unsubscribeTopology();
		this.#listeners.clear();
	}

	readonly #handleMainEvent = (event: DesktopApplicationLifetimeMainEvent): void => {
		if (this.#disposed) return;
		if (event.type === 'desktop-application-exit-requested') {
			if (this.#state.kind !== 'idle') return;
			this.#stopDurableRevision = null;
			const stopSnapshot = this.#runtime.getTerminalSessionStopSnapshot();
			const sessionCount = stopSnapshot.presentationCount - stopSnapshot.stoppedCount;
			if (event.reason === 'update-restart' || sessionCount === 0) {
				void this.#preparePreserve(event.requestId, event.reason === 'update-restart');
				return;
			}
			this.#setState({ kind: 'awaiting-choice', requestId: event.requestId, sessionCount });
			this.#postProgress(event.requestId, {
				completedCount: 0,
				failedCount: 0,
				phase: 'awaiting-choice',
				totalCount: sessionCount,
			});
			return;
		}
		if (this.#state.kind === 'idle' || event.requestId !== this.#state.requestId) return;
		if (event.result.kind === 'completed') {
			this.#setState({ kind: 'exiting', requestId: event.requestId });
			return;
		}
		this.#setState({
			failures: event.result.failures,
			kind: 'failed',
			requestId: event.requestId,
			retry: event.result.failures.every((failure) => failure.retryable) ? 'main' : null,
		});
	};

	async #preparePreserve(
		requestId: DesktopApplicationExitRequestId,
		bestEffort: boolean,
	): Promise<void> {
		this.#setWorking(requestId, 'flushing-topology', 0, 0, 0);
		const flushed = await this.#topology.flush();
		if (!this.#isActive(requestId)) return;
		const revision =
			flushed.kind === 'flushed' ? flushed.revision : this.#topology.committedRevision;
		if (revision === null || (flushed.kind === 'unavailable' && !bestEffort)) {
			this.#failRenderer(requestId, [
				{ code: 'topology-control-unavailable', count: 1, retryable: false },
			]);
			return;
		}
		this.#post({
			mode: 'preserve',
			presentationCount: this.#runtime.getTerminalSessionStopSnapshot().presentationCount,
			requestId,
			topologyRevision: revision,
			type: 'desktop-application-exit-renderer-prepared',
		});
	}

	async #beginStop(requestId: DesktopApplicationExitRequestId): Promise<void> {
		const result = this.#runtime.requestAllTerminalSessionsStop();
		if (result.kind === 'rejected') {
			this.#failRenderer(requestId, [{ code: 'terminal-stop-failed', count: 1, retryable: false }]);
			return;
		}
		this.#setWorking(
			requestId,
			'flushing-topology',
			result.snapshot.presentationCount,
			result.snapshot.stoppedCount,
			result.snapshot.failures.length,
		);
		await this.#flushStopWriteAhead(requestId);
	}

	async #flushStopWriteAhead(requestId: DesktopApplicationExitRequestId): Promise<void> {
		const flushed = await this.#topology.flush();
		if (!this.#isActive(requestId)) return;
		if (flushed.kind === 'unavailable') {
			this.#failRenderer(requestId, [
				{ code: 'topology-write-unavailable', count: 1, retryable: false },
			]);
			return;
		}
		this.#stopDurableRevision = flushed.revision;
		this.#observeStop();
	}

	readonly #observeStop = (): void => {
		if (
			this.#disposed ||
			this.#state.kind !== 'working' ||
			this.#stopDurableRevision === null ||
			this.#preparingStop
		) {
			return;
		}
		const snapshot = this.#runtime.getTerminalSessionStopSnapshot();
		if (snapshot.failures.length > 0) {
			this.#failStopSnapshot(this.#state.requestId, snapshot);
			return;
		}
		if (snapshot.complete) {
			this.#preparingStop = true;
			void this.#prepareCompletedStop(this.#state.requestId, snapshot);
			return;
		}
		this.#setWorking(
			this.#state.requestId,
			'stopping-terminals',
			snapshot.presentationCount,
			snapshot.stoppedCount,
			0,
		);
	};

	async #prepareCompletedStop(
		requestId: DesktopApplicationExitRequestId,
		snapshot: WorkbenchTerminalSessionStopSnapshot,
	): Promise<void> {
		const finalFlush = await this.#topology.flush();
		this.#preparingStop = false;
		if (!this.#isActive(requestId)) return;
		const revision =
			finalFlush.kind === 'flushed' ? finalFlush.revision : this.#stopDurableRevision;
		if (revision === null) {
			this.#failRenderer(requestId, [
				{ code: 'topology-write-unavailable', count: 1, retryable: false },
			]);
			return;
		}
		this.#setWorking(
			requestId,
			'draining-unreferenced',
			snapshot.presentationCount,
			snapshot.stoppedCount,
			0,
		);
		this.#post({
			mode: 'stop',
			presentationCount: snapshot.presentationCount,
			requestId,
			topologyRevision: revision,
			type: 'desktop-application-exit-renderer-prepared',
		});
	}

	#failStopSnapshot(
		requestId: DesktopApplicationExitRequestId,
		snapshot: WorkbenchTerminalSessionStopSnapshot,
	): void {
		this.#failRenderer(requestId, [
			{
				code: 'terminal-stop-failed',
				count: snapshot.failures.length,
				retryable: snapshot.failures.every((item) => item.failure.retryable),
			},
		]);
	}

	#failRenderer(
		requestId: DesktopApplicationExitRequestId,
		failures: readonly DesktopApplicationExitFailure[],
	): void {
		const retry =
			failures.every((failure) => failure.retryable) &&
			failures.every((failure) => failure.code === 'terminal-stop-failed')
				? 'terminal'
				: null;
		this.#setState({ failures, kind: 'failed', requestId, retry });
		this.#postProgress(
			requestId,
			{
				completedCount: 0,
				failedCount: failures.reduce((total, failure) => total + failure.count, 0),
				phase: 'failed',
				totalCount: this.#runtime.getTerminalSessionStopSnapshot().presentationCount,
			},
			failures,
		);
	}

	#setWorking(
		requestId: DesktopApplicationExitRequestId,
		phase: DesktopApplicationExitProgress['phase'],
		totalCount: number,
		completedCount: number,
		failedCount: number,
	): void {
		const progress = { completedCount, failedCount, phase, totalCount };
		this.#setState({ kind: 'working', progress, requestId });
		this.#postProgress(requestId, progress);
	}

	#postProgress(
		requestId: DesktopApplicationExitRequestId,
		progress: DesktopApplicationExitProgress,
		failures: readonly DesktopApplicationExitFailure[] = [],
	): void {
		this.#post({
			failures: [...failures],
			progress,
			requestId,
			type: 'desktop-application-exit-progress',
		});
	}

	#post(command: DesktopApplicationLifetimeRendererCommand): void {
		if (!this.#lifetimePort.post(command)) this.dispose();
	}

	#isActive(requestId: DesktopApplicationExitRequestId): boolean {
		return !this.#disposed && this.#state.kind !== 'idle' && this.#state.requestId === requestId;
	}

	#setState(state: DesktopApplicationLifetimeState): void {
		this.#state = state;
		for (const listener of this.#listeners) listener();
	}
}
