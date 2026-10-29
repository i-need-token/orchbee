import {
	measureMountedTerminalGeometry,
	type TerminalPresentationConfiguration,
} from '@orchbee/terminal';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { TerminalPresentationConfigurationSource } from '../../../terminal-presentation';
import {
	isTerminalThemePresentationSource,
	type TerminalThemePresentationParticipant,
} from '../../../terminal-settings';
import type {
	TerminalProjectionClient,
	TerminalProjectionRequest,
} from '../../terminal-projection';

type MeasureMountedTerminalGeometry = typeof measureMountedTerminalGeometry;

/** Owns mounted-geometry acquisition and the matching projection-open request. */
export class MountedTerminalProjectionRequest implements TerminalProjectionRequest {
	readonly #abortController = new AbortController();
	readonly #client: Pick<TerminalProjectionClient, 'open'>;
	readonly #configurationSource: TerminalPresentationConfigurationSource;
	#delegate: TerminalProjectionRequest | undefined;
	#initialConfiguration: TerminalPresentationConfiguration | undefined;
	readonly #measureGeometry: MeasureMountedTerminalGeometry;
	readonly #presentationId: TerminalPresentationId;
	#released = false;
	readonly #root: HTMLDivElement;
	readonly result: TerminalProjectionRequest['result'];

	constructor(
		root: HTMLDivElement | null,
		presentationId: TerminalPresentationId,
		client: Pick<TerminalProjectionClient, 'open'>,
		configurationSource: TerminalPresentationConfigurationSource,
		measureGeometry: MeasureMountedTerminalGeometry = measureMountedTerminalGeometry,
	) {
		if (root === null) throw new Error('The terminal viewport is not mounted.');
		this.#root = root;
		this.#presentationId = presentationId;
		this.#client = client;
		this.#configurationSource = configurationSource;
		this.#measureGeometry = measureGeometry;
		this.result = this.#measureAndOpen();
	}

	release(): void {
		if (this.#released) return;
		this.#released = true;
		this.#abortController.abort();
		this.#delegate?.release();
	}

	takePresentationConfigurationSource(): TerminalPresentationConfigurationSource {
		const initial = this.#initialConfiguration;
		if (initial === undefined) {
			throw new Error('Mounted terminal geometry has not been prepared.');
		}
		this.#initialConfiguration = undefined;
		const source = this.#configurationSource;
		return {
			current: () => initial,
			...(isTerminalThemePresentationSource(source)
				? {
						registerThemeParticipant: (participant: TerminalThemePresentationParticipant) =>
							source.registerThemeParticipant(participant),
					}
				: {}),
			subscribe: (listener) => {
				const unsubscribe = source.subscribe(listener);
				const latest = source.current();
				if (latest !== initial) listener(latest);
				return unsubscribe;
			},
		};
	}

	async #measureAndOpen(): TerminalProjectionRequest['result'] {
		while (true) {
			if (this.#abortController.signal.aborted) throw this.#measurementReleased();
			const configuration = this.#configurationSource.current();
			const geometry = await this.#measureGeometry(this.#root, configuration);
			if (this.#abortController.signal.aborted) throw this.#measurementReleased();
			if (geometry === undefined) {
				await this.#waitForMountedSurfaceChange();
				continue;
			}
			this.#initialConfiguration = configuration;
			const delegate = this.#client.open(this.#presentationId, geometry);
			this.#delegate = delegate;
			if (this.#released) delegate.release();
			return await delegate.result;
		}
	}

	#waitForMountedSurfaceChange(): Promise<void> {
		return new Promise((resolve, reject) => {
			let observer: ResizeObserver | undefined;
			let animationFrame: number | undefined;
			const signal = this.#abortController.signal;
			const view = this.#root.ownerDocument.defaultView;
			const cleanup = (): void => {
				observer?.disconnect();
				if (animationFrame !== undefined) view?.cancelAnimationFrame(animationFrame);
				signal.removeEventListener('abort', handleAbort);
			};
			const complete = (): void => {
				cleanup();
				resolve();
			};
			const handleAbort = (): void => {
				cleanup();
				reject(this.#measurementReleased());
			};
			signal.addEventListener('abort', handleAbort, { once: true });
			if (typeof ResizeObserver === 'function') {
				observer = new ResizeObserver(complete);
				observer.observe(this.#root);
			} else if (view !== null && typeof view.requestAnimationFrame === 'function') {
				animationFrame = view.requestAnimationFrame(complete);
			} else {
				cleanup();
				reject(new Error('Mounted terminal geometry observation is unavailable.'));
			}
			if (signal.aborted) handleAbort();
		});
	}

	#measurementReleased(): Error {
		return new Error('Terminal projection measurement was released.');
	}
}
