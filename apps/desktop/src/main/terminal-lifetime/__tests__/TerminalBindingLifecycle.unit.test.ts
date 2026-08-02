import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
	type TerminalExited,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { TerminalRecoveryOutcome } from '../../recovery';
import { TerminalBindingLifecycle } from '../TerminalBindingLifecycle';
import type { TerminalCloseOutcome, TerminalExitDisposition } from '../TerminalCloseOrchestrator';

const identity = {
	creationId: CreationIdSchema.parse('11'.repeat(16)),
	hostEpoch: HostEpochSchema.parse('22'.repeat(16)),
	terminalId: TerminalIdSchema.parse('33'.repeat(16)),
};
const exit: TerminalExited = {
	exit: { code: 0, kind: 'exit-code' },
	finalRevision: CanonicalUint64TextSchema.parse('4'),
	identity,
};

describe('TerminalBindingLifecycle', () => {
	it('resumes a durable close before exposing startup state', async () => {
		const recovery = createRecovery({ identity, kind: 'closing', summary: null });
		const closing = createClosing({ kind: 'closed' });
		const lifecycle = new TerminalBindingLifecycle(recovery, closing);

		expect(await lifecycle.reconcile()).toEqual({ kind: 'close-completed' });
		expect(closing.requestClose).toHaveBeenCalledOnce();
	});

	it('reports a safely pending resumed close without provisioning a replacement', async () => {
		const recovery = createRecovery({ identity, kind: 'closing', summary: null });
		const closing = createClosing({ kind: 'pending', reason: 'transport-unavailable' });
		const lifecycle = new TerminalBindingLifecycle(recovery, closing);

		expect(await lifecycle.reconcile()).toEqual({
			kind: 'close-pending',
			reason: 'transport-unavailable',
		});
		expect(recovery.reconcile).toHaveBeenCalledOnce();
	});

	it('serializes a close request before a later exit route', async () => {
		const trace: string[] = [];
		let releaseClose: (() => void) | undefined;
		const recovery = createRecovery({ identity, kind: 'provisioning', summary: null });
		const closing = {
			recordExit: vi.fn<(event: TerminalExited) => Promise<TerminalExitDisposition>>(async () => {
				trace.push('exit');
				return { handled: true, outcome: { kind: 'closed' } };
			}),
			requestClose: vi.fn<(retireProjection: () => Promise<void>) => Promise<TerminalCloseOutcome>>(
				async (retireProjection) => {
					trace.push('close:start');
					await retireProjection();
					await new Promise<void>((resolve) => {
						releaseClose = resolve;
					});
					trace.push('close:end');
					return { kind: 'closed' };
				},
			),
		};
		const lifecycle = new TerminalBindingLifecycle(recovery, closing);
		const close = lifecycle.requestClose(async () => {
			trace.push('projection:retired');
		});
		const recordExit = lifecycle.recordExit(exit);
		await vi.waitFor(() => expect(trace).toEqual(['close:start', 'projection:retired']));

		releaseClose?.();
		await Promise.all([close, recordExit]);
		expect(trace).toEqual(['close:start', 'projection:retired', 'close:end', 'exit']);
	});

	it('routes ordinary exits to recovery and consumes closing exits', async () => {
		const recovery = createRecovery({ identity, kind: 'provisioning', summary: null });
		const closing = createClosing({ kind: 'closed' }, { handled: false });
		const lifecycle = new TerminalBindingLifecycle(recovery, closing);

		await lifecycle.recordExit(exit);
		expect(recovery.recordExit).toHaveBeenCalledWith(exit);

		closing.recordExit.mockResolvedValueOnce({
			handled: true,
			outcome: { kind: 'closed' },
		});
		await lifecycle.recordExit(exit);
		expect(recovery.recordExit).toHaveBeenCalledOnce();
	});
});

function createRecovery(outcome: TerminalRecoveryOutcome) {
	return {
		reconcile: vi.fn<() => Promise<TerminalRecoveryOutcome>>().mockResolvedValue(outcome),
		recordExit: vi.fn<(event: TerminalExited) => Promise<void>>().mockResolvedValue(undefined),
	};
}

function createClosing(
	outcome: TerminalCloseOutcome,
	disposition: TerminalExitDisposition = { handled: true, outcome },
) {
	return {
		recordExit: vi
			.fn<(event: TerminalExited) => Promise<TerminalExitDisposition>>()
			.mockResolvedValue(disposition),
		requestClose: vi
			.fn<(retireProjection: () => Promise<void>) => Promise<TerminalCloseOutcome>>()
			.mockResolvedValue(outcome),
	};
}
