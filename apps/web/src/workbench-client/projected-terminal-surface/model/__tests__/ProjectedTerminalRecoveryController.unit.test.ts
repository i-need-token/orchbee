import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalRendererSession } from '../../../projected-terminal-renderer';
import type { TerminalProjection, TerminalProjectionRequest } from '../../../terminal-projection';
import {
	ProjectedTerminalRecoveryController,
	type ProjectedTerminalRecoveryState,
} from '../ProjectedTerminalRecoveryController';

describe('ProjectedTerminalRecoveryController', () => {
	it('routes a file drop only through the current painted attachment', async () => {
		const active = rendererSession();
		const request = projectionRequest();
		const states: ProjectedTerminalRecoveryState[] = [];
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: (state) => states.push(state),
			open: () => request.request,
			startRenderer: async () => active.session,
		});

		await expect(controller.pasteFileDrop("'/tmp/before.txt'")).resolves.toEqual({
			kind: 'unavailable',
		});
		controller.start();
		request.resolve();
		await vi.waitFor(() => expect(states.at(-1)).toEqual({ kind: 'ready' }));
		await expect(controller.pasteFileDrop("'/tmp/ready.txt'")).resolves.toMatchObject({
			kind: 'sent',
		});
		expect(active.session.pasteFileDrop).toHaveBeenCalledExactlyOnceWith("'/tmp/ready.txt'");

		controller.dispose();
		await expect(controller.pasteFileDrop("'/tmp/after.txt'")).resolves.toEqual({
			kind: 'unavailable',
		});
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
		expect(candidate.dispose).toHaveBeenCalledOnce();
		expect(candidate.freeze).not.toHaveBeenCalled();
		expect(request.release).toHaveBeenCalledOnce();
		expect(states.at(-1)).toEqual({ attempt: 1, kind: 'recovering' });
		controller.dispose();
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
	readonly dispose: ReturnType<typeof vi.fn<() => void>>;
	readonly freeze: ReturnType<typeof vi.fn<() => void>>;
	readonly session: ProjectedTerminalRendererSession;
} {
	const dispose = vi.fn<() => void>();
	const freeze = vi.fn<() => void>();
	const pasteFileDrop = vi.fn<ProjectedTerminalRendererSession['pasteFileDrop']>(async () => ({
		kind: 'sent',
		review: {
			bracketed: false,
			embeddedBracketTerminator: false,
			multiline: false,
			originalText: "'/tmp/file.txt'",
			replacedControlCharacters: 0,
			source: 'file-drop',
			text: "'/tmp/file.txt'",
		},
	}));
	return {
		dispose,
		freeze,
		session: {
			dispose,
			freeze,
			isTransportAvailable: () => true,
			pasteFileDrop,
			presentationReady: Promise.resolve(),
			transportUnavailable: new Promise<void>(() => undefined),
			updatePresentationConfiguration:
				vi.fn<ProjectedTerminalRendererSession['updatePresentationConfiguration']>(),
			...overrides,
		},
	};
}

function projectionRequest(): {
	readonly release: ReturnType<typeof vi.fn<() => void>>;
	readonly request: TerminalProjectionRequest;
	resolve(): void;
} {
	const projection = deferred<TerminalProjection>();
	const release = vi.fn<() => void>();
	return {
		release,
		request: { release, result: projection.promise },
		resolve: () =>
			projection.resolve({
				bootstrap: {} as TerminalProjection['bootstrap'],
				transport: {} as TerminalProjection['transport'],
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
