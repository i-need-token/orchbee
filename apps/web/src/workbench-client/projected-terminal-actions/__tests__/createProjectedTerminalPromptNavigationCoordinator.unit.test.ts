import { describe, expect, it, vi } from 'vitest';

import { createProjectedTerminalPromptNavigationCoordinator } from '../createProjectedTerminalPromptNavigationCoordinator';

describe('createProjectedTerminalPromptNavigationCoordinator', () => {
	it('returns the terminal navigation outcome for one current request', async () => {
		const coordinator = createProjectedTerminalPromptNavigationCoordinator();
		const navigate = vi
			.fn<(signal: AbortSignal) => Promise<boolean>>()
			.mockResolvedValueOnce(true)
			.mockResolvedValueOnce(false);
		const signal = new AbortController().signal;

		await expect(coordinator.run(signal, navigate)).resolves.toBe('moved');
		await expect(coordinator.run(signal, navigate)).resolves.toBe('unchanged');
		expect(navigate).toHaveBeenCalledTimes(2);
	});

	it('aborts an older direction when a newer request starts', async () => {
		const coordinator = createProjectedTerminalPromptNavigationCoordinator();
		let firstSignal: AbortSignal | undefined;
		const first = coordinator.run(
			new AbortController().signal,
			(signal) =>
				new Promise((resolve) => {
					firstSignal = signal;
					signal.addEventListener('abort', () => resolve(false), { once: true });
				}),
		);

		const second = coordinator.run(new AbortController().signal, async () => true);
		await expect(first).resolves.toBe('cancelled');
		await expect(second).resolves.toBe('moved');
		expect(firstSignal?.aborted).toBe(true);
	});

	it('forwards caller cancellation and aborts the active request on disposal', async () => {
		const coordinator = createProjectedTerminalPromptNavigationCoordinator();
		const caller = new AbortController();
		let callerSignal: AbortSignal | undefined;
		const callerRequest = coordinator.run(
			caller.signal,
			(signal) =>
				new Promise((resolve) => {
					callerSignal = signal;
					signal.addEventListener('abort', () => resolve(false), { once: true });
				}),
		);
		caller.abort();
		await expect(callerRequest).resolves.toBe('cancelled');
		expect(callerSignal?.aborted).toBe(true);

		let disposedSignal: AbortSignal | undefined;
		const disposedRequest = coordinator.run(
			new AbortController().signal,
			(signal) =>
				new Promise((resolve) => {
					disposedSignal = signal;
					signal.addEventListener('abort', () => resolve(false), { once: true });
				}),
		);
		coordinator.dispose();
		await expect(disposedRequest).resolves.toBe('cancelled');
		expect(disposedSignal?.aborted).toBe(true);
	});

	it('retains the latest navigation started by a superseded request cancellation', async () => {
		const coordinator = createProjectedTerminalPromptNavigationCoordinator();
		const caller = new AbortController().signal;
		let latest: ReturnType<typeof coordinator.run> | undefined;
		let latestSignal: AbortSignal | undefined;
		const first = coordinator.run(
			caller,
			(signal) =>
				new Promise((resolve) => {
					signal.addEventListener('abort', () => {
						latest = coordinator.run(caller, async (currentSignal) => {
							latestSignal = currentSignal;
							return true;
						});
						resolve(false);
					});
				}),
		);
		const supersededNavigate = vi
			.fn<(signal: AbortSignal) => Promise<boolean>>()
			.mockResolvedValue(true);
		const superseded = coordinator.run(caller, supersededNavigate);
		coordinator.dispose();

		expect(supersededNavigate).not.toHaveBeenCalled();
		expect(latestSignal?.aborted).toBe(true);
		await expect(Promise.all([first, superseded, latest])).resolves.toEqual([
			'cancelled',
			'cancelled',
			'cancelled',
		]);
	});

	it('does not navigate when previous cancellation disposes the owner', async () => {
		const coordinator = createProjectedTerminalPromptNavigationCoordinator();
		const caller = new AbortController().signal;
		const first = coordinator.run(
			caller,
			(signal) =>
				new Promise((resolve) => {
					signal.addEventListener('abort', () => {
						coordinator.dispose();
						resolve(false);
					});
				}),
		);
		const navigate = vi.fn<(signal: AbortSignal) => Promise<boolean>>().mockResolvedValue(true);
		const second = coordinator.run(caller, navigate);
		const outcomes = await Promise.all([first, second]);
		coordinator.dispose();

		expect(navigate).not.toHaveBeenCalled();
		expect(outcomes).toEqual(['cancelled', 'cancelled']);
	});

	it('rejects navigation attempted during disposal and after the registration lifetime', async () => {
		const coordinator = createProjectedTerminalPromptNavigationCoordinator();
		const caller = new AbortController().signal;
		const navigate = vi.fn<(signal: AbortSignal) => Promise<boolean>>().mockResolvedValue(true);
		let duringDisposal: ReturnType<typeof coordinator.run> | undefined;
		const active = coordinator.run(
			caller,
			(signal) =>
				new Promise((resolve) => {
					signal.addEventListener('abort', () => {
						duringDisposal = coordinator.run(caller, navigate);
						resolve(false);
					});
				}),
		);
		coordinator.dispose();
		const afterDisposal = coordinator.run(caller, navigate);
		const outcomes = await Promise.all([active, duringDisposal, afterDisposal]);
		coordinator.dispose();

		expect(navigate).not.toHaveBeenCalled();
		expect(outcomes).toEqual(['cancelled', 'cancelled', 'cancelled']);
	});

	it('keeps current navigation when a caller is already cancelled at admission', async () => {
		const coordinator = createProjectedTerminalPromptNavigationCoordinator();
		let activeSignal: AbortSignal | undefined;
		const active = coordinator.run(new AbortController().signal, async (signal) => {
			activeSignal = signal;
			return true;
		});
		const cancelledCaller = new AbortController();
		cancelledCaller.abort();
		const navigate = vi.fn<(signal: AbortSignal) => Promise<boolean>>().mockResolvedValue(true);
		const cancelled = coordinator.run(cancelledCaller.signal, navigate);
		const wasActiveAborted = activeSignal?.aborted;
		const outcomes = await Promise.all([active, cancelled]);
		coordinator.dispose();

		expect(wasActiveAborted).toBe(false);
		expect(navigate).not.toHaveBeenCalled();
		expect(outcomes).toEqual(['moved', 'cancelled']);
	});
});
