import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
	MobileTerminalInputState,
	ProjectedTerminalRendererSession,
} from '../../../projected-terminal-renderer';
import type { TerminalProjection, TerminalProjectionRequest } from '../../../terminal-projection';
import {
	ProjectedTerminalRecoveryController,
	type ProjectedTerminalRecoveryControllerOptions,
	type ProjectedTerminalRecoveryState,
} from '../ProjectedTerminalRecoveryController';

afterEach(() => vi.useRealTimers());

describe('ProjectedTerminalRecoveryController', () => {
	it.each(['subscribeControlPresentation', 'subscribeMobileInputState'] as const)(
		'honors %s cancellation during the final disposal notification',
		(subscribe) => {
			const controller = new ProjectedTerminalRecoveryController({
				onStateChange: () => undefined,
				open: vi.fn<ProjectedTerminalRecoveryControllerOptions['open']>(),
				startRenderer: vi.fn<ProjectedTerminalRecoveryControllerOptions['startRenderer']>(),
			});
			const listener = vi.fn<() => void>();
			controller[subscribe](() => subscription.dispose());
			const subscription = controller[subscribe](listener);
			controller.dispose();
			expect(listener).not.toHaveBeenCalled();
		},
	);

	it.each(['dispose', 'retain'] as const)(
		'releases a projection acquired while open synchronously calls %s',
		(retirement) => {
			const release = vi.fn<() => void>();
			const startRenderer = vi.fn<ProjectedTerminalRecoveryControllerOptions['startRenderer']>();
			const controller: ProjectedTerminalRecoveryController =
				new ProjectedTerminalRecoveryController({
					onStateChange: () => undefined,
					open: () => {
						controller[retirement]();
						return { release, result: new Promise(() => undefined) };
					},
					startRenderer,
				});
			controller.start();
			expect(release).toHaveBeenCalledOnce();
			expect(startRenderer).not.toHaveBeenCalled();
			controller.dispose();
			expect(release).toHaveBeenCalledOnce();
		},
	);

	it.each(['dispose', 'retain'] as const)(
		'leaves no retry timer when the recovery observer calls %s',
		(retirement) => {
			vi.useFakeTimers();
			const open = vi.fn<() => TerminalProjectionRequest>(() => {
				throw new Error('Attachment unavailable');
			});
			const controller: ProjectedTerminalRecoveryController =
				new ProjectedTerminalRecoveryController({
					onStateChange: (state) => {
						if (state.kind === 'recovering') controller[retirement]();
					},
					open,
					startRenderer: vi.fn<ProjectedTerminalRecoveryControllerOptions['startRenderer']>(),
				});
			controller.start();
			expect(vi.getTimerCount()).toBe(0);
			expect(open).toHaveBeenCalledOnce();
			controller.dispose();
		},
	);

	it.each(['dispose', 'retain'] as const)(
		'stops promotion notifications when the ready observer calls %s',
		async (retirement) => {
			const events: string[] = [];
			const active = rendererSession();
			const request = projectionRequest();
			const controller: ProjectedTerminalRecoveryController =
				new ProjectedTerminalRecoveryController({
					onActivationReady: () => events.push('activated'),
					onDisplayReady: () => events.push('displayed'),
					onStateChange: (state) => {
						if (state.kind !== 'ready') return;
						events.push('retired');
						controller[retirement]();
					},
					open: () => request.request,
					startRenderer: async () => active.session,
				});
			controller.start();
			request.resolve();
			await vi.waitFor(() => expect(events).toContain('retired'));
			expect(events.slice(events.indexOf('retired') + 1)).toEqual([]);
			expect(request.release).toHaveBeenCalledOnce();
			controller.dispose();
			expect(active.dispose).toHaveBeenCalledOnce();
		},
	);

	it('preserves a replacement opened during synchronous retention and resumption', async () => {
		const retiredRelease = vi.fn<() => void>();
		const replacement = projectionRequest();
		const active = rendererSession();
		const states: ProjectedTerminalRecoveryState[] = [];
		const open = vi
			.fn<() => TerminalProjectionRequest>()
			.mockImplementationOnce(() => {
				controller.retain();
				controller.resume();
				return { release: retiredRelease, result: new Promise(() => undefined) };
			})
			.mockReturnValueOnce(replacement.request);
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: (state) => states.push(state),
			open,
			startRenderer: async () => active.session,
		});
		controller.start();
		expect(open).toHaveBeenCalledTimes(2);
		expect(retiredRelease).toHaveBeenCalledOnce();
		expect(replacement.release).not.toHaveBeenCalled();
		replacement.resolve();
		await vi.waitFor(() => expect(states.at(-1)).toEqual({ kind: 'ready' }));
		controller.dispose();
		expect(replacement.release).toHaveBeenCalledOnce();
		expect(retiredRelease).toHaveBeenCalledOnce();
	});

	it('admits one request when opening synchronously reenters start', () => {
		const request = projectionRequest();
		const open = vi
			.fn<() => TerminalProjectionRequest>()
			.mockImplementationOnce(() => {
				controller.start();
				return request.request;
			})
			.mockReturnValue(request.request);
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: () => undefined,
			open,
			startRenderer: vi.fn<ProjectedTerminalRecoveryControllerOptions['startRenderer']>(),
		});
		controller.start();
		expect(open).toHaveBeenCalledOnce();
		controller.dispose();
		expect(request.release).toHaveBeenCalledOnce();
	});

	it('closes a late transport from a request retired during acquisition', async () => {
		const request = projectionRequest();
		const startRenderer = vi.fn<ProjectedTerminalRecoveryControllerOptions['startRenderer']>();
		const controller: ProjectedTerminalRecoveryController = new ProjectedTerminalRecoveryController(
			{
				onStateChange: () => undefined,
				open: () => {
					controller.retain();
					return request.request;
				},
				startRenderer,
			},
		);
		controller.start();
		request.resolve();
		await vi.waitFor(() => expect(request.closeTransport).toHaveBeenCalledOnce());
		expect(startRenderer).not.toHaveBeenCalled();
		expect(request.release).toHaveBeenCalledOnce();
		controller.dispose();
	});

	it('consumes cancellation rejection from a request retired during acquisition', async () => {
		const release = vi.fn<() => void>();
		const startRenderer = vi.fn<ProjectedTerminalRecoveryControllerOptions['startRenderer']>();
		const controller: ProjectedTerminalRecoveryController = new ProjectedTerminalRecoveryController(
			{
				onStateChange: () => undefined,
				open: () => {
					controller.dispose();
					return { release, result: Promise.reject(new Error('Projection was released')) };
				},
				startRenderer,
			},
		);
		controller.start();
		// Let the test runner observe an unhandled rejection if cancellation was not consumed.
		await new Promise<void>((resolve) => globalThis.setTimeout(resolve, 0));
		expect(release).toHaveBeenCalledOnce();
		expect(startRenderer).not.toHaveBeenCalled();
	});

	it('releases an input subscription acquired during disposal', async () => {
		const disposeMobile = vi.fn<() => void>();
		const subscribeControl =
			vi.fn<NonNullable<ProjectedTerminalRendererSession['subscribeControlPresentation']>>();
		const activationReady = vi.fn<() => void>();
		const active = rendererSession({
			subscribeControlPresentation: subscribeControl,
			subscribeMobileInputState: () => {
				controller.dispose();
				return { dispose: disposeMobile };
			},
		});
		const request = projectionRequest();
		const controller = new ProjectedTerminalRecoveryController({
			onActivationReady: activationReady,
			onStateChange: () => undefined,
			open: () => request.request,
			startRenderer: async () => active.session,
		});
		controller.start();
		request.resolve();
		await vi.waitFor(() => expect(active.dispose).toHaveBeenCalledOnce());
		expect(disposeMobile).toHaveBeenCalledOnce();
		expect(subscribeControl).not.toHaveBeenCalled();
		expect(activationReady).not.toHaveBeenCalled();
		expect(request.release).toHaveBeenCalledOnce();
	});

	it('keeps candidate ownership while retiring the previous frozen renderer', async () => {
		const firstRequest = projectionRequest();
		const secondRequest = projectionRequest();
		const first = rendererSession();
		const second = rendererSession();
		const open = vi
			.fn<() => TerminalProjectionRequest>()
			.mockReturnValueOnce(firstRequest.request)
			.mockReturnValueOnce(secondRequest.request);
		const startRenderer = vi
			.fn<ProjectedTerminalRecoveryControllerOptions['startRenderer']>()
			.mockResolvedValueOnce(first.session)
			.mockResolvedValueOnce(second.session);
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: () => undefined,
			open,
			startRenderer,
		});
		controller.start();
		firstRequest.resolve();
		await vi.waitFor(() => expect(controller.focusTerminal()).toBe(true));
		controller.retain();
		first.dispose.mockImplementation(() => controller.dispose());
		controller.resume();
		secondRequest.resolve();
		await vi.waitFor(() => expect(first.dispose).toHaveBeenCalled());
		expect(first.dispose).toHaveBeenCalledOnce();
		expect(second.dispose).toHaveBeenCalledOnce();
		expect(secondRequest.release).toHaveBeenCalledOnce();
		expect(controller.focusTerminal()).toBe(false);
	});

	it('disposes a retiring renderer when freezing synchronously disposes its owner', async () => {
		const request = projectionRequest();
		const active = rendererSession();
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: () => undefined,
			open: () => request.request,
			startRenderer: async () => active.session,
		});
		controller.start();
		request.resolve();
		await vi.waitFor(() => expect(controller.focusTerminal()).toBe(true));
		active.freeze.mockImplementation(() => controller.dispose());
		controller.retain();
		expect(active.dispose).toHaveBeenCalledOnce();
		expect(request.release).toHaveBeenCalledOnce();
		controller.dispose();
		expect(active.dispose).toHaveBeenCalledOnce();
	});

	it.each(['retain', 'transport-loss'] as const)(
		'does not freeze a disposed renderer when %s releases its input subscription',
		async (retirement) => {
			vi.useFakeTimers();
			const unavailable = deferred<void>();
			const request = projectionRequest();
			const subscriptionDispose = vi.fn<() => void>(() => controller.dispose());
			const active = rendererSession({
				subscribeMobileInputState: () => ({ dispose: subscriptionDispose }),
				transportUnavailable: unavailable.promise,
			});
			const controller = new ProjectedTerminalRecoveryController({
				onStateChange: () => undefined,
				open: () => request.request,
				startRenderer: async () => active.session,
			});
			controller.start();
			request.resolve();
			await vi.waitFor(() => expect(controller.focusTerminal()).toBe(true));
			if (retirement === 'retain') controller.retain();
			else {
				unavailable.resolve();
				await drainMicrotasks();
			}
			expect(subscriptionDispose).toHaveBeenCalledOnce();
			expect(active.dispose).toHaveBeenCalledOnce();
			expect(active.freeze).not.toHaveBeenCalled();
			expect(request.release).toHaveBeenCalledOnce();
			expect(vi.getTimerCount()).toBe(0);
		},
	);

	it('keeps a replacement requested while releasing a retained pending attempt', () => {
		const first = projectionRequest();
		const second = projectionRequest();
		const open = vi
			.fn<() => TerminalProjectionRequest>()
			.mockReturnValueOnce(first.request)
			.mockReturnValueOnce(second.request);
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: () => undefined,
			open,
			startRenderer: vi.fn<ProjectedTerminalRecoveryControllerOptions['startRenderer']>(),
		});
		controller.start();
		first.release.mockImplementation(() => controller.resume());
		controller.retain();
		expect(open).toHaveBeenCalledTimes(2);
		expect(first.release).toHaveBeenCalledOnce();
		expect(second.release).not.toHaveBeenCalled();
		controller.dispose();
		expect(second.release).toHaveBeenCalledOnce();
	});

	it('admits a replacement when retiring an input subscription resumes the surface', async () => {
		const firstRequest = projectionRequest();
		const secondRequest = projectionRequest();
		const first = rendererSession({
			subscribeMobileInputState: () => ({ dispose: () => controller.resume() }),
		});
		const second = rendererSession();
		const open = vi
			.fn<() => TerminalProjectionRequest>()
			.mockReturnValueOnce(firstRequest.request)
			.mockReturnValueOnce(secondRequest.request);
		const startRenderer = vi
			.fn<ProjectedTerminalRecoveryControllerOptions['startRenderer']>()
			.mockResolvedValueOnce(first.session)
			.mockResolvedValueOnce(second.session);
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: () => undefined,
			open,
			startRenderer,
		});
		controller.start();
		firstRequest.resolve();
		await vi.waitFor(() => expect(controller.focusTerminal()).toBe(true));
		controller.retain();
		expect(open).toHaveBeenCalledTimes(2);
		expect(first.freeze).toHaveBeenCalledOnce();
		expect(firstRequest.release).toHaveBeenCalledOnce();
		secondRequest.resolve();
		await vi.waitFor(() => expect(controller.focusTerminal()).toBe(true));
		expect(first.dispose).toHaveBeenCalledOnce();
		controller.dispose();
		expect(second.dispose).toHaveBeenCalledOnce();
		expect(secondRequest.release).toHaveBeenCalledOnce();
	});

	it('releases every owned resource and retains cleanup failures', async () => {
		const controlError = new Error('Control cleanup failed');
		const mobileError = new Error('Mobile cleanup failed');
		const rendererError = new Error('Renderer cleanup failed');
		const releaseError = new Error('Request cleanup failed');
		const disposeControl = vi.fn<() => void>(() => {
			throw controlError;
		});
		const disposeMobile = vi.fn<() => void>(() => {
			throw mobileError;
		});
		const active = rendererSession({
			subscribeControlPresentation: () => ({ dispose: disposeControl }),
			subscribeMobileInputState: () => ({ dispose: disposeMobile }),
		});
		const request = projectionRequest();
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: () => undefined,
			open: () => request.request,
			startRenderer: async () => active.session,
		});
		controller.start();
		request.resolve();
		await vi.waitFor(() => expect(controller.focusTerminal()).toBe(true));
		active.dispose.mockImplementation(() => {
			throw rendererError;
		});
		request.release.mockImplementation(() => {
			throw releaseError;
		});
		let failure: unknown;
		try {
			controller.dispose();
		} catch (error) {
			failure = error;
		}
		expect(failure).toBeInstanceOf(AggregateError);
		if (!(failure instanceof AggregateError)) throw failure;
		expect(disposeControl).toHaveBeenCalledOnce();
		expect(disposeMobile).toHaveBeenCalledOnce();
		expect(active.dispose).toHaveBeenCalledOnce();
		expect(request.release).toHaveBeenCalledOnce();
		expect(collectFailures(failure)).toEqual([
			controlError,
			mobileError,
			rendererError,
			releaseError,
		]);
		controller.dispose();
		expect(active.dispose).toHaveBeenCalledOnce();
		expect(request.release).toHaveBeenCalledOnce();
	});

	it('retires the active attachment and resumes behind its frozen frame', async () => {
		const displayReady = vi.fn<() => void>();
		const activationReady = vi.fn<() => void>();
		const first = rendererSession();
		const secondDisplay = deferred<void>();
		const secondActivation = deferred<void>();
		const second = rendererSession({
			displayReady: secondDisplay.promise,
			presentationReady: secondActivation.promise,
		});
		const firstRequest = projectionRequest();
		const secondRequest = projectionRequest();
		const open = vi
			.fn<() => TerminalProjectionRequest>()
			.mockReturnValueOnce(firstRequest.request)
			.mockReturnValueOnce(secondRequest.request);
		const startRenderer = vi
			.fn<(projection: TerminalProjection) => Promise<ProjectedTerminalRendererSession>>()
			.mockResolvedValueOnce(first.session)
			.mockResolvedValueOnce(second.session);
		const controller = new ProjectedTerminalRecoveryController({
			onActivationReady: activationReady,
			onDisplayReady: displayReady,
			onStateChange: () => undefined,
			open,
			startRenderer,
		});

		controller.start();
		firstRequest.resolve();
		await drainMicrotasks();
		displayReady.mockClear();
		activationReady.mockClear();
		controller.retain();

		expect(first.capturePresentationSnapshot).toHaveBeenCalledOnce();
		expect(first.freeze).toHaveBeenCalledOnce();
		expect(firstRequest.release).toHaveBeenCalledOnce();
		expect(controller.focusTerminal()).toBe(false);

		controller.resume();
		expect(displayReady).toHaveBeenCalledOnce();
		expect(open).toHaveBeenCalledTimes(2);
		secondRequest.resolve();
		await drainMicrotasks();
		secondDisplay.resolve();
		await drainMicrotasks();
		expect(first.dispose).not.toHaveBeenCalled();
		expect(activationReady).not.toHaveBeenCalled();

		secondActivation.resolve();
		await drainMicrotasks();
		expect(first.dispose).toHaveBeenCalledOnce();
		expect(activationReady).toHaveBeenCalledOnce();
		controller.dispose();
	});

	it('retains a display-ready candidate when transport fails before activation', async () => {
		const displayReady = deferred<void>();
		const activationReady = deferred<void>();
		const transportUnavailable = deferred<void>();
		const candidate = rendererSession({
			displayReady: displayReady.promise,
			presentationReady: activationReady.promise,
			transportUnavailable: transportUnavailable.promise,
		});
		const request = projectionRequest();
		const controller = new ProjectedTerminalRecoveryController({
			onDisplayReady: vi.fn<() => void>(),
			onStateChange: () => undefined,
			open: () => request.request,
			startRenderer: async () => candidate.session,
		});

		controller.start();
		request.resolve();
		await drainMicrotasks();
		displayReady.resolve();
		await drainMicrotasks();
		transportUnavailable.resolve();
		await drainMicrotasks();

		expect(candidate.freeze).toHaveBeenCalledOnce();
		expect(candidate.dispose).not.toHaveBeenCalled();
		expect(request.release).toHaveBeenCalledOnce();
		controller.dispose();
		expect(candidate.dispose).toHaveBeenCalledOnce();
	});

	it('focuses only the current painted attachment', async () => {
		const active = rendererSession();
		const request = projectionRequest();
		const states: ProjectedTerminalRecoveryState[] = [];
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: (state) => states.push(state),
			open: () => request.request,
			startRenderer: async () => active.session,
		});

		expect(controller.focusTerminal()).toBe(false);
		controller.start();
		request.resolve();
		await vi.waitFor(() => expect(states.at(-1)).toEqual({ kind: 'ready' }));
		expect(controller.focusTerminal()).toBe(true);
		expect(active.focus).toHaveBeenCalledOnce();

		controller.dispose();
		expect(controller.focusTerminal()).toBe(false);
		expect(active.focus).toHaveBeenCalledOnce();
	});

	it('routes mobile input state and controls only through the exact active renderer', async () => {
		let mobileState: MobileTerminalInputState = {
			alt: false,
			control: false,
			focused: true,
			readiness: { kind: 'ready', revision: 'attachment-mobile:5' },
		};
		let emitMobileState = (): void => undefined;
		const disposeMobileState = vi.fn<() => void>();
		const sendMobileKey = vi.fn<NonNullable<ProjectedTerminalRendererSession['sendMobileKey']>>(
			() => true,
		);
		const toggleMobileKeyboard = vi.fn<
			NonNullable<ProjectedTerminalRendererSession['toggleMobileKeyboard']>
		>(() => true);
		const toggleMobileModifier =
			vi.fn<NonNullable<ProjectedTerminalRendererSession['toggleMobileModifier']>>();
		const active = rendererSession({
			getMobileInputState: () => mobileState,
			sendMobileKey,
			subscribeMobileInputState: (listener) => {
				emitMobileState = listener;
				return { dispose: disposeMobileState };
			},
			toggleMobileKeyboard,
			toggleMobileModifier,
		});
		const request = projectionRequest();
		const states: ProjectedTerminalRecoveryState[] = [];
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: (state) => states.push(state),
			open: () => request.request,
			startRenderer: async () => active.session,
		});
		const mobileStateChanged = vi.fn<() => void>();
		controller.subscribeMobileInputState(mobileStateChanged);

		expect(controller.getMobileInputState().readiness).toEqual({ kind: 'unavailable' });
		expect(controller.sendMobileKey('Escape')).toBe(false);
		controller.start();
		request.resolve();
		await vi.waitFor(() => expect(states.at(-1)).toEqual({ kind: 'ready' }));

		expect(controller.getMobileInputState()).toEqual(mobileState);
		expect(controller.sendMobileKey('ArrowUp')).toBe(true);
		expect(controller.toggleMobileKeyboard()).toBe(true);
		controller.toggleMobileModifier('control');
		expect(sendMobileKey).toHaveBeenCalledExactlyOnceWith('ArrowUp');
		expect(toggleMobileKeyboard).toHaveBeenCalledOnce();
		expect(toggleMobileModifier).toHaveBeenCalledExactlyOnceWith('control');

		mobileState = {
			alt: false,
			control: false,
			focused: false,
			readiness: { kind: 'ready', revision: 'attachment-mobile:6' },
		};
		emitMobileState();
		expect(controller.getMobileInputState()).toEqual(mobileState);
		expect(mobileStateChanged).toHaveBeenCalledTimes(2);

		controller.dispose();
		expect(disposeMobileState).toHaveBeenCalledOnce();
		expect(controller.getMobileInputState().readiness).toEqual({ kind: 'unavailable' });
	});

	it('retains the active frame across bounded recovery and replaces it only after a painted frame', async () => {
		vi.useFakeTimers();
		const states: ProjectedTerminalRecoveryState[] = [];
		const activeUnavailable = deferred<void>();
		const candidateReady = deferred<void>();
		const active = rendererSession({ transportUnavailable: activeUnavailable.promise });
		const candidate = rendererSession({ presentationReady: candidateReady.promise });
		const first = projectionRequest();
		const second = projectionRequest();
		const third = projectionRequest();
		const open = vi
			.fn<() => TerminalProjectionRequest>()
			.mockReturnValueOnce(first.request)
			.mockReturnValueOnce(second.request)
			.mockReturnValueOnce(third.request);
		const startRenderer = vi
			.fn<(projection: TerminalProjection) => Promise<ProjectedTerminalRendererSession>>()
			.mockResolvedValueOnce(active.session)
			.mockRejectedValueOnce(new Error('still unavailable'))
			.mockResolvedValueOnce(candidate.session);
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: (state) => states.push(state),
			open,
			startRenderer,
		});

		controller.start();
		first.resolve();
		await drainMicrotasks();
		activeUnavailable.resolve();
		await drainMicrotasks();
		expect(active.freeze).toHaveBeenCalledOnce();
		expect(active.dispose).not.toHaveBeenCalled();
		expect(first.release).toHaveBeenCalledOnce();

		await vi.advanceTimersByTimeAsync(250);
		second.resolve();
		await drainMicrotasks();
		await vi.advanceTimersByTimeAsync(500);
		third.resolve();
		await drainMicrotasks();
		expect(active.dispose).not.toHaveBeenCalled();

		candidateReady.resolve();
		await drainMicrotasks();
		expect(active.dispose).toHaveBeenCalledOnce();
		expect(states.at(-1)).toEqual({ kind: 'ready' });

		controller.dispose();
		expect(candidate.dispose).toHaveBeenCalledOnce();
		expect(third.release).toHaveBeenCalledOnce();
		vi.useRealTimers();
	});

	it('rejects a painted candidate that disconnects before renderer startup returns', async () => {
		vi.useFakeTimers();
		const states: ProjectedTerminalRecoveryState[] = [];
		const candidateReady = deferred<void>();
		const candidateUnavailable = deferred<void>();
		let candidateTransportAvailable = true;
		const candidate = rendererSession({
			isTransportAvailable: () => candidateTransportAvailable,
			presentationReady: candidateReady.promise,
			transportUnavailable: candidateUnavailable.promise,
		});
		const request = projectionRequest();
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: (state) => states.push(state),
			open: () => request.request,
			startRenderer: async () => {
				candidateReady.resolve();
				await Promise.resolve();
				candidateTransportAvailable = false;
				candidateUnavailable.resolve();
				await Promise.resolve();
				return candidate.session;
			},
		});

		controller.start();
		request.resolve();
		await drainMicrotasks();

		expect(candidate.dispose).toHaveBeenCalledOnce();
		expect(candidate.freeze).not.toHaveBeenCalled();
		expect(request.release).toHaveBeenCalledOnce();
		expect(states.at(-1)).toEqual({ attempt: 1, kind: 'recovering' });
		controller.dispose();
		vi.useRealTimers();
	});

	it('rechecks candidate transport availability after the ready race settles', async () => {
		vi.useFakeTimers();
		const states: ProjectedTerminalRecoveryState[] = [];
		const isTransportAvailable = vi.fn<() => boolean>();
		isTransportAvailable.mockReturnValueOnce(true).mockReturnValue(false);
		const candidate = rendererSession({ isTransportAvailable });
		const request = projectionRequest();
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: (state) => states.push(state),
			open: () => request.request,
			startRenderer: async () => candidate.session,
		});

		controller.start();
		request.resolve();
		await drainMicrotasks();
		await drainMicrotasks();

		expect(isTransportAvailable).toHaveBeenCalledTimes(2);
		expect(candidate.dispose).not.toHaveBeenCalled();
		expect(candidate.freeze).toHaveBeenCalledOnce();
		expect(request.release).toHaveBeenCalledOnce();
		expect(states.at(-1)).toEqual({ attempt: 1, kind: 'recovering' });
		controller.dispose();
		expect(candidate.dispose).toHaveBeenCalledOnce();
		vi.useRealTimers();
	});

	it('exhausts three automatic retries and allows one immediate manual attempt', async () => {
		vi.useFakeTimers();
		const states: ProjectedTerminalRecoveryState[] = [];
		const releases: Array<ReturnType<typeof vi.fn<() => void>>> = [];
		const open = vi.fn<() => TerminalProjectionRequest>(() => {
			const release = vi.fn<() => void>();
			releases.push(release);
			return { release, result: Promise.reject(new Error('unavailable')) };
		});
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: (state) => states.push(state),
			open,
			startRenderer: () => {
				throw new Error('Rejected requests must not start a renderer.');
			},
		});

		controller.start();
		await drainMicrotasks();
		for (const delay of [250, 500, 1_000]) {
			await vi.advanceTimersByTimeAsync(delay);
			await drainMicrotasks();
		}
		expect(open).toHaveBeenCalledTimes(4);
		expect(states.at(-1)).toEqual({ kind: 'failed' });
		expect(releases.every((release) => release.mock.calls.length === 1)).toBe(true);

		controller.retry();
		await drainMicrotasks();
		expect(open).toHaveBeenCalledTimes(5);
		expect(states.at(-1)).toEqual({ kind: 'failed' });
		expect(vi.getTimerCount()).toBe(0);
		controller.dispose();
		vi.useRealTimers();
	});
});

function rendererSession(overrides: Partial<ProjectedTerminalRendererSession> = {}): {
	readonly capturePresentationSnapshot: ReturnType<typeof vi.fn<() => null>>;
	readonly dispose: ReturnType<typeof vi.fn<() => void>>;
	readonly focus: ReturnType<typeof vi.fn<() => void>>;
	readonly freeze: ReturnType<typeof vi.fn<() => void>>;
	readonly session: ProjectedTerminalRendererSession;
} {
	const dispose = vi.fn<() => void>();
	const capturePresentationSnapshot = vi.fn<() => null>(() => null);
	const focus = vi.fn<() => void>();
	const freeze = vi.fn<() => void>();
	return {
		capturePresentationSnapshot,
		dispose,
		focus,
		freeze,
		session: {
			capturePresentationSnapshot,
			dispose,
			focus,
			freeze,
			isTransportAvailable: () => true,
			presentationReady: Promise.resolve(),
			transportUnavailable: new Promise<void>(() => undefined),
			updatePresentationConfiguration:
				vi.fn<ProjectedTerminalRendererSession['updatePresentationConfiguration']>(),
			...overrides,
		},
	};
}

function projectionRequest(): {
	readonly closeTransport: ReturnType<typeof vi.fn<() => void>>;
	readonly release: ReturnType<typeof vi.fn<() => void>>;
	readonly request: TerminalProjectionRequest;
	resolve(): void;
} {
	const projection = deferred<TerminalProjection>();
	const closeTransport = vi.fn<() => void>();
	const release = vi.fn<() => void>();
	return {
		closeTransport,
		release,
		request: { release, result: projection.promise },
		resolve: () =>
			projection.resolve({
				bootstrap: {} as TerminalProjection['bootstrap'],
				transport: {
					close: closeTransport,
					projectionBufferTrust: 'local-host',
					send: vi.fn<TerminalProjection['transport']['send']>(),
					subscribe: () => () => undefined,
				},
			}),
	};
}

function deferred<T>(): {
	readonly promise: Promise<T>;
	resolve(value: T extends void ? never : T): void;
	resolve(): void;
} {
	let resolvePromise: ((value: T) => void) | undefined;
	const promise = new Promise<T>((resolve) => {
		resolvePromise = resolve;
	});
	return {
		promise,
		resolve: (value?: T) => resolvePromise?.(value as T),
	};
}

async function drainMicrotasks(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}

function collectFailures(error: unknown): unknown[] {
	return error instanceof AggregateError
		? error.errors.flatMap((failure: unknown) => collectFailures(failure))
		: [error];
}
