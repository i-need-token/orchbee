import { CreationIdSchema } from '@orchbee/terminal-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import type { AgentHookRichObservationEnvelope } from '../agent-hook-observation-contract';
import {
	AgentObservationJoinStateMachine,
	type AgentObservationJoinCommand,
	type AgentObservationJoinStateMachineOptions,
} from '../AgentObservationJoinStateMachine';
import { resolveAgentHookAdapter } from '../resolveAgentHookAdapter';

describe('AgentObservationJoinStateMachine', () => {
	it('admits one exact event in either half-arrival order', () => {
		const richFirst = machine();
		const hintFirst = machine();
		const richFirstPresentation = register(richFirst, 'rich-first', '11');
		const hintFirstPresentation = register(hintFirst, 'hint-first', '22');

		expect(richFirst.receiveEvent(adapter(), envelope(1), 0)).toEqual([]);
		const richFirstCommands = richFirst.receiveHint(hint(richFirstPresentation, '11', 1, 1), 1);
		expect(hintFirst.receiveHint(hint(hintFirstPresentation, '22', 1, 1), 0)).toEqual([]);
		const hintFirstCommands = hintFirst.receiveEvent(adapter(), envelope(1), 1);

		expect(commandTypes(richFirstCommands)).toEqual(['apply-observation']);
		expect(commandTypes(hintFirstCommands)).toEqual(['apply-observation']);
		expect(applyCommand(richFirstCommands).attachments).toEqual([
			expect.objectContaining({ presentationId: richFirstPresentation }),
		]);
		expect(applyCommand(hintFirstCommands).attachments).toEqual([
			expect.objectContaining({ presentationId: hintFirstPresentation }),
		]);
	});

	it('settles once and turns later correlations into attachment-only fan-out', () => {
		const state = machine();
		const firstPresentation = register(state, 'first', '33');
		const secondPresentation = register(state, 'second', '44');
		state.receiveEvent(adapter(), envelope(1, [1, 2], ['hidden', 'visible']), 0);

		const scheduled = state.receiveHint(hint(firstPresentation, '33', 1, 1), 1);
		expect(commandTypes(scheduled)).toEqual(['schedule-settlement']);
		const schedule = scheduled[0];
		if (schedule?.type !== 'schedule-settlement') {
			throw new Error('Expected a settlement command.');
		}
		const settled = state.settle(schedule.eventKey, schedule.eventExpiresAt, 2);
		expect(commandTypes(settled)).toEqual(['apply-observation']);

		const late = state.receiveHint(hint(secondPresentation, '44', 2, 1), 3);
		expect(commandTypes(late)).toEqual(['attach-presentations']);
		expect(late[0]).toMatchObject({
			attachments: [{ presentationId: secondPresentation, visibility: 'visible' }],
			type: 'attach-presentations',
		});
	});

	it('combines visibility and cancels settlement when the final correlation arrives', () => {
		const state = machine();
		const presentationId = register(state, 'nested', '55');
		state.receiveHint(hint(presentationId, '55', 1, 1), 0);
		state.receiveHint(hint(presentationId, '55', 2, 1), 0);

		const commands = state.receiveEvent(adapter(), envelope(1, [1, 2], ['visible', 'hidden']), 1);

		expect(commandTypes(commands)).toEqual([
			'schedule-settlement',
			'cancel-settlement',
			'apply-observation',
		]);
		expect(applyCommand(commands).attachments).toEqual([
			expect.objectContaining({ presentationId, visibility: 'hidden' }),
		]);
	});

	it('preserves an event generation fence while retiring presentations independently', () => {
		const state = machine();
		const firstPresentation = register(state, 'replace', '66');
		const secondPresentation = register(state, 'stable', '77');
		state.receiveEvent(adapter(), envelope(1, [1, 2]), 0);
		state.receiveHint(hint(firstPresentation, '66', 1, 1), 1);
		const retired = state.registerPresentationGeneration(
			firstPresentation,
			CreationIdSchema.parse('88'.repeat(16)),
		);
		expect(commandTypes(retired)).toEqual(['retire-presentation-generation']);

		expect(state.receiveHint(hint(firstPresentation, '88', 2, 1), 2)).toEqual([]);
		const remaining = state.receiveHint(hint(secondPresentation, '77', 2, 1), 3);
		expect(commandTypes(remaining)).toEqual(['cancel-settlement', 'apply-observation']);
		expect(applyCommand(remaining).attachments).toEqual([
			expect.objectContaining({ presentationId: secondPresentation }),
		]);
	});

	it('rejects expired and stale settlement incarnations without canonical output', () => {
		const state = machine({ joinTtlMilliseconds: 10 });
		const presentationId = register(state, 'expiry', '99');
		const richEvent = envelope(1, [1, 2]);
		state.receiveEvent(adapter(), richEvent, 0);
		const scheduled = state.receiveHint(hint(presentationId, '99', 1, 1), 1);
		const firstSchedule = scheduled[0];
		if (firstSchedule?.type !== 'schedule-settlement') {
			throw new Error('Expected a settlement command.');
		}

		expect(commandTypes(state.receiveEvent(adapter(), richEvent, 10))).toEqual([
			'cancel-settlement',
		]);
		const replacementSchedule = state.receiveHint(hint(presentationId, '99', 1, 1), 11)[0];
		if (replacementSchedule?.type !== 'schedule-settlement') {
			throw new Error('Expected a replacement settlement command.');
		}
		expect(state.settle(firstSchedule.eventKey, firstSchedule.eventExpiresAt, 12)).toEqual([]);
		expect(
			commandTypes(
				state.settle(replacementSchedule.eventKey, replacementSchedule.eventExpiresAt, 12),
			),
		).toEqual(['apply-observation']);
		state.reset();
		expect(
			state.settle(replacementSchedule.eventKey, replacementSchedule.eventExpiresAt, 13),
		).toEqual([]);
	});

	it('keeps exact duplicates idempotent and rejects conflicting event identities', () => {
		const state = machine();
		const presentationId = register(state, 'duplicate', 'aa');
		const richEvent = envelope(1, [1, 2]);
		state.receiveEvent(adapter(), richEvent, 0);
		expect(commandTypes(state.receiveHint(hint(presentationId, 'aa', 1, 1), 1))).toEqual([
			'schedule-settlement',
		]);

		expect(
			state.receiveEvent(
				adapter(),
				{ ...richEvent, correlations: richEvent.correlations.slice(0, 1) },
				2,
			),
		).toEqual([]);
		const completed = state.receiveHint(hint(presentationId, 'aa', 2, 1), 3);
		expect(commandTypes(completed)).toEqual(['cancel-settlement', 'apply-observation']);
		expect(state.receiveEvent(adapter(), richEvent, 4)).toEqual([]);
		expect(state.receiveHint(hint(presentationId, 'aa', 1, 1), 4)).toEqual([]);
	});

	it('fails closed on correlation collisions and bounded pending capacity', () => {
		const collisionState = machine();
		const collisionPresentation = register(collisionState, 'collision', 'bb');
		collisionState.receiveEvent(adapter(), envelope(1, [1, 2]), 0);
		expect(collisionState.admitEvent(adapter(), envelope(2, [1]), 1)).toEqual({
			accepted: false,
			commands: [],
		});
		expect(
			commandTypes(collisionState.receiveHint(hint(collisionPresentation, 'bb', 1, 1), 2)),
		).toEqual(['schedule-settlement']);

		const eventCapacityState = machine({ maxPendingEvents: 1 });
		const eventCapacityPresentation = register(eventCapacityState, 'event-capacity', 'cc');
		eventCapacityState.receiveEvent(adapter(), envelope(1), 0);
		expect(eventCapacityState.admitEvent(adapter(), envelope(2), 1)).toEqual({
			accepted: false,
			commands: [],
		});
		expect(
			commandTypes(eventCapacityState.receiveHint(hint(eventCapacityPresentation, 'cc', 1, 1), 2)),
		).toEqual(['apply-observation']);

		const hintCapacityState = machine({ maxPendingHints: 1 });
		const hintCapacityPresentation = register(hintCapacityState, 'hint-capacity', 'dd');
		hintCapacityState.receiveHint(hint(hintCapacityPresentation, 'dd', 1, 1), 0);
		hintCapacityState.receiveHint(hint(hintCapacityPresentation, 'dd', 2, 2), 1);
		expect(hintCapacityState.receiveEvent(adapter(), envelope(2), 2)).toEqual([]);
		expect(commandTypes(hintCapacityState.receiveEvent(adapter(), envelope(1), 3))).toEqual([
			'apply-observation',
		]);
	});
});

function machine(
	options: AgentObservationJoinStateMachineOptions = {},
): AgentObservationJoinStateMachine {
	return new AgentObservationJoinStateMachine(options);
}

function register(state: AgentObservationJoinStateMachine, name: string, generationByte: string) {
	const presentationId = terminalPresentationId(`desktop:presentation:${name}`);
	state.registerPresentationGeneration(
		presentationId,
		CreationIdSchema.parse(generationByte.repeat(16)),
	);
	return presentationId;
}

function adapter() {
	const value = resolveAgentHookAdapter('claude-code', 'turn-started');
	if (value === null) throw new Error('Expected the Claude Code test adapter.');
	return value;
}

function envelope(
	sequence: number,
	correlations: readonly number[] = [sequence],
	visibilities: readonly ('hidden' | 'unknown' | 'visible')[] = correlations.map(() => 'hidden'),
): AgentHookRichObservationEnvelope {
	return {
		adapterId: 'claude-code',
		correlations: correlations.map((value, index) => ({
			correlationId: correlationId(value),
			visibility: visibilities[index] ?? 'unknown',
		})),
		eventId: eventId(sequence),
		observation: {
			activity: 'thinking',
			agentKey: null,
			event: 'turn-started',
			operationKey: null,
			sessionKey: 'claude-session-1',
			turnKey: null,
			version: 1,
		},
		version: 3,
	};
}

function hint(
	presentationId: ReturnType<typeof terminalPresentationId>,
	generationByte: string,
	correlationSequence: number,
	eventSequence: number,
) {
	return {
		correlationId: correlationId(correlationSequence),
		eventId: eventId(eventSequence),
		generation: CreationIdSchema.parse(generationByte.repeat(16)),
		presentationId,
	};
}

function applyCommand(commands: readonly AgentObservationJoinCommand[]) {
	const command = commands.find((candidate) => candidate.type === 'apply-observation');
	if (command === undefined) throw new Error('Expected an apply-observation command.');
	return command;
}

function commandTypes(commands: readonly AgentObservationJoinCommand[]): readonly string[] {
	return commands.map((command) => command.type);
}

function eventId(sequence: number): string {
	return `event-${String(sequence).padStart(16, '0')}`;
}

function correlationId(sequence: number): string {
	return `correlation-${String(sequence).padStart(16, '0')}`;
}
