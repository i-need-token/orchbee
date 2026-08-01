import { describe, expect, it } from 'vitest';

import {
	createInitialProtocolLifecycleState,
	HostPhase,
	LifecycleOutcome,
	transitionProtocolLifecycle,
} from '../lifecycle';

describe('protocol lifecycle transition', () => {
	it('starts one epoch from the serializable absent state', () => {
		const initial = createInitialProtocolLifecycleState();
		expect(JSON.parse(JSON.stringify(initial))).toEqual(initial);
		const transition = transitionProtocolLifecycle(initial, {
			epoch: '11111111111111111111111111111111',
			type: 'launch-host',
		});
		expect(transition).toEqual(
			expect.objectContaining({
				ok: true,
				value: expect.objectContaining({
					outcome: LifecycleOutcome.applied,
					state: expect.objectContaining({ phase: HostPhase.starting }),
				}),
			}),
		);
	});
});
