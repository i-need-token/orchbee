import { TerminalApplicationSignalSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { DesktopAgentObservationRuntime } from '../DesktopAgentObservationRuntime';

describe('DesktopAgentObservationRuntime', () => {
	it('owns volatile observations independently of subscribers and retires terminal hints by presentation', () => {
		let eventSequence = 0;
		const runtime = new DesktopAgentObservationRuntime({
			allocateEventId: () => `desktop-event-${++eventSequence}`,
		});
		runtime.observeTerminalApplicationSignal(activitySignal(), 'presentation-1');
		const listener = vi.fn<() => void>();
		const unsubscribe = runtime.subscribe(listener);

		runtime.retireTerminalSignalPresentation('presentation-2');
		expect(runtime.getSnapshot().sessions[0]?.presentationAttachments[0]?.navigable).toBe(true);

		runtime.retireTerminalSignalPresentation('presentation-1');
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			observation: { freshness: 'disconnected' },
			presentationAttachments: [{ navigable: false, presentationId: 'presentation-1' }],
		});
		expect(listener).toHaveBeenCalledOnce();
		unsubscribe();
	});

	it('reduces OSC application signals to ordered generic activity hints only', () => {
		let eventSequence = 0;
		const runtime = new DesktopAgentObservationRuntime({
			allocateEventId: () => `desktop-event-${++eventSequence}`,
		});
		const signal = activitySignal();

		runtime.observeTerminalApplicationSignal(signal, 'presentation-1');
		expect(runtime.getSnapshot().sessions).toEqual([
			expect.objectContaining({
				activityHint: 'command',
				capabilities: ['activity'],
				lastOutcome: null,
				lifecycle: 'live',
				observation: expect.objectContaining({ fidelity: 'basic', source: 'terminal' }),
				pendingRequests: [],
				providerId: 'terminal-signal',
				turn: { kind: 'unknown' },
			}),
		]);

		const revision = runtime.getSnapshot().revision;
		runtime.observeTerminalApplicationSignal(signal, 'presentation-1');
		runtime.observeTerminalApplicationSignal(
			TerminalApplicationSignalSchema.parse({
				...signal,
				payload: JSON.stringify({ activity: 'thinking', idle: true, version: 1 }),
				sequence: '2',
			}),
			'presentation-1',
		);
		expect(runtime.getSnapshot().revision).toBe(revision);
	});
});

function activitySignal() {
	return TerminalApplicationSignalSchema.parse({
		channel: 'orchbee.agent.v1',
		identity: {
			creationId: '11223344556677889900aabbccddeeff',
			hostEpoch: '00112233445566778899aabbccddeeff',
			terminalId: 'ffeeddccbbaa99887766554433221100',
		},
		payload: JSON.stringify({ activity: 'command', version: 1 }),
		sequence: '1',
	});
}
