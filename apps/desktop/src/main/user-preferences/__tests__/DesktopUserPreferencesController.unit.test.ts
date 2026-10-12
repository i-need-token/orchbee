import {
	DEFAULT_USER_PREFERENCES,
	UserPreferencesSnapshotSchema,
	type UserPreferencesRequest,
} from '@orchbee/workbench-client-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	DesktopUserPreferencesController,
	type DesktopUserPreferencesObservation,
} from '../DesktopUserPreferencesController';
import type { UserPreferencesExchange } from '../NativeUserPreferencesClient';

const source = 'a'.repeat(64);
const saved = UserPreferencesSnapshotSchema.parse({
	source,
	values: DEFAULT_USER_PREFERENCES,
	claimed: [],
	desktopImported: true,
	desktopMainImported: true,
});
const observed: UserPreferencesExchange = {
	kind: 'response',
	response: { kind: 'snapshot', snapshot: saved, changed: false },
};

afterEach(() => vi.useRealTimers());

describe('Desktop shared preference publication', () => {
	it('coalesces reads and hands a frozen write off once after polling completes', async () => {
		let finish: ((value: UserPreferencesExchange) => void) | undefined;
		const execute = vi
			.fn<(request: UserPreferencesRequest) => Promise<UserPreferencesExchange>>()
			.mockImplementationOnce(
				() =>
					new Promise((resolve) => {
						finish = resolve;
					}),
			)
			.mockResolvedValue({ kind: 'outcome-unknown' });
		const controller = new DesktopUserPreferencesController({
			execute,
			dispose: async () => undefined,
		});
		const poll = controller.execute({ operation: 'read' });
		expect(controller.execute({ operation: 'read' })).toBe(poll);
		const request: UserPreferencesRequest = {
			operation: 'update',
			source: 'missing',
			changes: { soundEnabled: true },
		};
		const save = controller.execute(request);
		expect(execute).toHaveBeenCalledTimes(1);
		finish?.(observed);
		expect((await save).response.kind).toBe('outcome-unknown');
		expect(execute.mock.calls).toEqual([[{ operation: 'read' }], [request]]);
		await controller.dispose();
	});

	it('applies an already imported snapshot when the Host joins, then only changed sources', async () => {
		vi.useFakeTimers();
		const execute = vi.fn<(request: UserPreferencesRequest) => Promise<UserPreferencesExchange>>(
			async () => observed,
		);
		const dispose = vi.fn<() => Promise<void>>(async () => undefined);
		const controller = new DesktopUserPreferencesController({ execute, dispose });
		const apply = vi.fn<() => Promise<void>>(async () => undefined);
		await controller.execute({ operation: 'import-desktop', part: 'main', changes: {} });
		await controller.start(apply);
		expect(apply).toHaveBeenCalledExactlyOnceWith(saved);
		await vi.advanceTimersByTimeAsync(1000);
		expect(apply).toHaveBeenCalledOnce();
		await controller.dispose();
		expect(dispose).toHaveBeenCalledOnce();
		const count = execute.mock.calls.length;
		await vi.advanceTimersByTimeAsync(5000);
		expect(execute).toHaveBeenCalledTimes(count);
	});

	it('reports a saved source separately from rejected Host application and retries only publication', async () => {
		vi.useFakeTimers();
		const execute = vi.fn<(request: UserPreferencesRequest) => Promise<UserPreferencesExchange>>(
			async () => observed,
		);
		const controller = new DesktopUserPreferencesController({
			execute,
			dispose: async () => undefined,
		});
		const apply = vi
			.fn<() => Promise<void>>()
			.mockRejectedValueOnce(new Error('Host disconnected'))
			.mockResolvedValue(undefined);
		const listener = vi.fn<(observation: DesktopUserPreferencesObservation) => void>();
		controller.subscribe(listener);
		await controller.start(apply);
		expect(listener).toHaveBeenLastCalledWith({
			response: observed.response,
			applicationIssue: 'host-rejected',
			legacyIssue: false,
		});
		expect(controller.getSnapshot()).toEqual(saved);
		await vi.advanceTimersByTimeAsync(1000);
		expect(listener).toHaveBeenLastCalledWith({
			response: observed.response,
			applicationIssue: null,
			legacyIssue: false,
		});
		expect(execute.mock.calls.every(([request]) => request.operation === 'read')).toBe(true);
		await controller.dispose();
	});

	it('observes after an unknown commit without replaying or rolling back the edit', async () => {
		vi.useFakeTimers();
		const execute = vi.fn<(request: UserPreferencesRequest) => Promise<UserPreferencesExchange>>(
			async (request) => (request.operation === 'update' ? { kind: 'outcome-unknown' } : observed),
		);
		const controller = new DesktopUserPreferencesController({
			execute,
			dispose: async () => undefined,
		});
		await controller.start(async () => undefined);
		expect(
			await controller.execute({
				operation: 'update',
				source,
				changes: { applicationTheme: 'dark' },
			}),
		).toMatchObject({ response: { kind: 'outcome-unknown' } });
		await vi.advanceTimersByTimeAsync(3000);
		expect(execute.mock.calls.filter(([request]) => request.operation === 'update')).toHaveLength(
			1,
		);
		await controller.dispose();
	});
});
