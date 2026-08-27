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
});
