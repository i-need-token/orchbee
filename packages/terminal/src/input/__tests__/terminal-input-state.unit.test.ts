import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

import {
	beginTerminalComposition,
	createTerminalInputState,
	disposeTerminalInputState,
	finishTerminalComposition,
	observeTerminalBeforeInput,
	observeTerminalPaste,
	recordTerminalBeforeInput,
	recordTerminalComposition,
	recordTerminalKeyInput,
	recordTerminalMouseSample,
	recordTerminalPaste,
	repairTerminalInputState,
	supersedeCancelledTerminalComposition,
	TerminalBeforeInputOrigin,
	terminalCompositionUpdateIsEligible,
} from '../terminal-input-state';

const deduplicationWindow = 100;

type ConformanceRecordKind = 'beforeinput' | 'clipboard' | 'composition' | 'key';
type ConformanceCheckKind = 'beforeinput' | 'compositionend';

interface ConformanceEvent<Kind extends string> {
	readonly kind: Kind;
	readonly text: string;
	readonly time: number;
}

interface ConformanceTrace {
	readonly checks: readonly ConformanceEvent<ConformanceCheckKind>[];
	readonly duplicates: readonly boolean[];
	readonly name: string;
	readonly record: ConformanceEvent<ConformanceRecordKind>;
}

interface InputRepairConformance {
	readonly format: string;
	readonly source: {
		readonly commit: string;
		readonly deduplicationWindowMilliseconds: number;
		readonly path: string;
	};
	readonly traces: readonly ConformanceTrace[];
	readonly version: string;
}

function parseInputRepairConformance(value: unknown): InputRepairConformance {
	const root = exactObject(value, ['format', 'source', 'traces', 'version']);
	const source = exactObject(root['source'], ['commit', 'deduplicationWindowMilliseconds', 'path']);
	if (!Array.isArray(root['traces'])) throw new Error('traces must be an array');
	return {
		format: requiredString(root['format'], 'format'),
		source: {
			commit: requiredString(source['commit'], 'source.commit'),
			deduplicationWindowMilliseconds: requiredNonnegativeInteger(
				source['deduplicationWindowMilliseconds'],
				'source.deduplicationWindowMilliseconds',
			),
			path: requiredString(source['path'], 'source.path'),
		},
		traces: root['traces'].map((trace, index) => parseConformanceTrace(trace, index)),
		version: requiredString(root['version'], 'version'),
	};
}

function parseConformanceTrace(value: unknown, index: number): ConformanceTrace {
	const trace = exactObject(value, ['checks', 'duplicates', 'name', 'record']);
	if (!Array.isArray(trace['checks'])) throw new Error(`traces[${index}].checks must be an array`);
	if (!Array.isArray(trace['duplicates'])) {
		throw new Error(`traces[${index}].duplicates must be an array`);
	}
	if (
		trace['duplicates'].length !== trace['checks'].length ||
		!trace['duplicates'].every((duplicate) => typeof duplicate === 'boolean')
	) {
		throw new Error(`traces[${index}].duplicates must match checks`);
	}
	return {
		checks: trace['checks'].map((check) =>
			parseConformanceEvent(check, ['beforeinput', 'compositionend']),
		),
		duplicates: trace['duplicates'],
		name: requiredString(trace['name'], `traces[${index}].name`),
		record: parseConformanceEvent(trace['record'], [
			'beforeinput',
			'clipboard',
			'composition',
			'key',
		]),
	};
}

function parseConformanceEvent<Kind extends string>(
	value: unknown,
	allowedKinds: readonly Kind[],
): ConformanceEvent<Kind> {
	if (typeof value !== 'string') throw new Error('conformance event must be a string');
	const match = /^(?<kind>[a-z]+):(?<text>.+)@(?<time>[0-9]+)$/u.exec(value);
	const kind = match?.groups?.['kind'];
	const text = match?.groups?.['text'];
	const timeText = match?.groups?.['time'];
	const allowedKind = allowedKinds.find((candidate) => candidate === kind);
	if (!allowedKind || !text || !timeText) {
		throw new Error(`unsupported conformance event: ${value}`);
	}
	return {
		kind: allowedKind,
		text,
		time: requiredNonnegativeInteger(Number(timeText), 'event time'),
	};
}

function exactObject(value: unknown, keys: readonly string[]): Record<string, unknown> {
	if (value === null || typeof value !== 'object' || Array.isArray(value)) {
		throw new Error('conformance value must be an object');
	}
	const record = value as Record<string, unknown>;
	const actualKeys = Object.keys(record).sort();
	const expectedKeys = [...keys].sort();
	if (
		actualKeys.length !== expectedKeys.length ||
		actualKeys.some((key, index) => key !== expectedKeys[index])
	) {
		throw new Error(`unexpected conformance fields: ${actualKeys.join(',')}`);
	}
	return record;
}

function requiredString(value: unknown, field: string): string {
	if (typeof value !== 'string' || value.length === 0) throw new Error(`${field} must be a string`);
	return value;
}

function requiredNonnegativeInteger(value: unknown, field: string): number {
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
		throw new Error(`${field} must be a nonnegative safe integer`);
	}
	return value;
}

function replayConformanceTrace(trace: ConformanceTrace, window: number): readonly boolean[] {
	let state = createTerminalInputState<string>();
	switch (trace.record.kind) {
		case 'beforeinput':
			state = recordTerminalBeforeInput(beginTerminalComposition(state), trace.record);
			break;
		case 'clipboard':
			state = recordTerminalPaste(state, { ...trace.record, source: 'clipboard' });
			break;
		case 'composition':
			state = recordTerminalComposition(state, trace.record);
			break;
		case 'key':
			state = recordTerminalKeyInput(state, trace.record);
			break;
	}
	return trace.checks.map((check) => {
		if (trace.record.kind === 'clipboard') {
			if (check.kind !== 'beforeinput') throw new Error('clipboard trace requires beforeinput');
			const decision = observeTerminalPaste(state, check.text, 'beforeinput', check.time, window);
			state = decision.state;
			return decision.duplicate;
		}
		if (check.kind === 'beforeinput') {
			const decision = observeTerminalBeforeInput(state, check.text, check.time, window);
			state = decision.state;
			return decision.duplicate;
		}
		if (trace.record.kind !== 'beforeinput') {
			throw new Error('compositionend trace requires beforeinput record');
		}
		const hadCorrelation = state.beforeInput !== undefined;
		const decision = finishTerminalComposition(state, check.text, check.time, window);
		state = decision.state;
		return hadCorrelation && state.beforeInput === undefined;
	});
}

describe('terminal input state', () => {
	it('replays pinned mature input-correlation trace version 0.0.1', () => {
		const rawProvenance: unknown = JSON.parse(
			readFileSync(
				new URL('../../../docs/input-repair-conformance-v0.0.1.json', import.meta.url),
				'utf8',
			),
		);
		const provenance = parseInputRepairConformance(rawProvenance);
		expect(provenance).toMatchObject({
			format: 'orchbee-terminal-input-repair-conformance',
			source: {
				commit: 'be7b4d0d531be6bab4c6cb4fb164addb997c951d',
				deduplicationWindowMilliseconds: deduplicationWindow,
				path: 'packages/terminal/src/input/InputHandler.ts',
			},
			traces: [
				{ duplicates: [true, false], name: 'key-beforeinput-single-use' },
				{ duplicates: [true, false], name: 'composition-beforeinput-single-use' },
				{ duplicates: [true, false], name: 'beforeinput-composition-single-use' },
				{ duplicates: [true, false], name: 'cross-source-paste-single-use' },
			],
			version: '0.0.1',
		});

		for (const trace of provenance.traces) {
			expect(replayConformanceTrace(trace, deduplicationWindow)).toEqual(trace.duplicates);
		}
	});

	it('rejects unknown conformance fields and event forms', () => {
		expect(() =>
			parseInputRepairConformance({
				format: 'orchbee-terminal-input-repair-conformance',
				source: {
					commit: 'commit',
					deduplicationWindowMilliseconds: 100,
					path: 'path',
				},
				traces: [],
				unexpected: true,
				version: '0.0.1',
			}),
		).toThrow(/unexpected conformance fields/u);
		expect(() =>
			parseInputRepairConformance({
				format: 'orchbee-terminal-input-repair-conformance',
				source: {
					commit: 'commit',
					deduplicationWindowMilliseconds: 100,
					path: 'path',
				},
				traces: [
					{
						checks: ['unknown:a@1'],
						duplicates: [false],
						name: 'unknown',
						record: 'key:a@0',
					},
				],
				version: '0.0.1',
			}),
		).toThrow(/unsupported conformance event/u);
		expect(() =>
			parseInputRepairConformance({
				format: 'orchbee-terminal-input-repair-conformance',
				source: {
					commit: 'commit',
					deduplicationWindowMilliseconds: 100,
					path: 'path',
				},
				traces: [
					{
						checks: ['beforeinput:a@1'],
						duplicates: [false],
						name: 'unknown',
						record: 'unknown:a@0',
					},
				],
				version: '0.0.1',
			}),
		).toThrow(/unsupported conformance event/u);
	});

	it('separates cancelled native composition work from local publication eligibility', () => {
		const eligible = beginTerminalComposition(createTerminalInputState<string>());
		expect(terminalCompositionUpdateIsEligible(eligible)).toBe(true);

		const repair = repairTerminalInputState(eligible);
		expect(repair.state).toMatchObject({
			compositionPhase: 'cancelled-awaiting-end',
		});
		expect(terminalCompositionUpdateIsEligible(repair.state)).toBe(false);
		const lateEnd = finishTerminalComposition(repair.state, '候选', 1, deduplicationWindow);
		expect(lateEnd).toMatchObject({
			publishText: false,
			state: { compositionPhase: 'idle' },
		});
		expect(
			finishTerminalComposition(lateEnd.state, '候选', 2, deduplicationWindow).publishText,
		).toBe(false);
	});

	it('suppresses cancelled composition in both beforeinput and end orders', () => {
		const cancelled = repairTerminalInputState(
			beginTerminalComposition(createTerminalInputState<string>()),
		).state;
		const endFirst = finishTerminalComposition(cancelled, '候选', 1, deduplicationWindow);
		expect(endFirst).toMatchObject({
			publishText: false,
			state: {
				compositionPhase: 'idle',
				compositionSettlement: { kind: 'awaiting-beforeinput' },
			},
		});
		const trailingBeforeInput = observeTerminalBeforeInput(
			endFirst.state,
			'候选',
			1_000,
			deduplicationWindow,
			TerminalBeforeInputOrigin.composition,
		);
		expect(trailingBeforeInput).toMatchObject({ duplicate: true, publishText: false });
		expect(trailingBeforeInput.state.compositionSettlement).toBeUndefined();

		const beforeFirst = observeTerminalBeforeInput(
			cancelled,
			'候选',
			10,
			deduplicationWindow,
			TerminalBeforeInputOrigin.composition,
		).state;
		expect(beforeFirst.compositionSettlement).toEqual({
			beforeInputObserved: true,
			kind: 'awaiting-native-end',
		});
		const trailingEnd = finishTerminalComposition(beforeFirst, '候选', 1_000, deduplicationWindow);
		expect(trailingEnd).toMatchObject({
			publishText: false,
			state: {
				beforeInput: undefined,
				composition: undefined,
				compositionPhase: 'idle',
				compositionSettlement: undefined,
			},
		});
	});

	it('keeps settlement across repair while allowing composition supersession and independent input', () => {
		const normalEnd = finishTerminalComposition(
			beginTerminalComposition(createTerminalInputState<string>()),
			'same',
			1,
			deduplicationWindow,
		);
		expect(normalEnd.publishText).toBe(true);
		const twiceRepaired = repairTerminalInputState(
			repairTerminalInputState(normalEnd.state).state,
		).state;
		const trailing = observeTerminalBeforeInput(
			twiceRepaired,
			'same',
			1_000,
			deduplicationWindow,
			TerminalBeforeInputOrigin.composition,
		);
		expect(trailing).toMatchObject({ duplicate: true, publishText: false });
		expect(
			observeTerminalBeforeInput(
				trailing.state,
				'same',
				1_000,
				deduplicationWindow,
				TerminalBeforeInputOrigin.independent,
			).publishText,
		).toBe(true);

		const cancelled = repairTerminalInputState(
			beginTerminalComposition(createTerminalInputState<string>()),
		).state;
		const superseded = beginTerminalComposition(cancelled);
		expect(superseded).toMatchObject({
			compositionPhase: 'eligible',
			compositionSettlement: undefined,
		});
		expect(finishTerminalComposition(superseded, 'new', 2, deduplicationWindow).publishText).toBe(
			true,
		);

		const independentlyProgressing = supersedeCancelledTerminalComposition(cancelled);
		expect(independentlyProgressing).toMatchObject({
			compositionPhase: 'idle',
			compositionSettlement: { kind: 'awaiting-native-end' },
		});
		const mobile = observeTerminalBeforeInput(
			cancelled,
			'mobile',
			3,
			deduplicationWindow,
			TerminalBeforeInputOrigin.independent,
		);
		expect(mobile).toMatchObject({ publishText: true, state: { compositionPhase: 'idle' } });
		const delayedComposition = observeTerminalBeforeInput(
			mobile.state,
			'same',
			4,
			deduplicationWindow,
			TerminalBeforeInputOrigin.composition,
		);
		expect(delayedComposition.publishText).toBe(false);
		expect(
			observeTerminalBeforeInput(
				delayedComposition.state,
				'same',
				4,
				deduplicationWindow,
				TerminalBeforeInputOrigin.independent,
			).publishText,
		).toBe(true);
	});

	it('commits at most once and deduplicates both composition event orders', () => {
		const eligible = beginTerminalComposition(createTerminalInputState<string>());
		const endFirst = finishTerminalComposition(eligible, '新', 10, deduplicationWindow);
		expect(endFirst.publishText).toBe(true);
		expect(endFirst.state).toMatchObject({
			composition: { text: '新', time: 10 },
			compositionSettlement: { kind: 'awaiting-beforeinput' },
		});
		const trailingBeforeInput = observeTerminalBeforeInput(
			endFirst.state,
			'新',
			11,
			deduplicationWindow,
		);
		expect(trailingBeforeInput.duplicate).toBe(true);
		expect(
			finishTerminalComposition(endFirst.state, '新', 12, deduplicationWindow).publishText,
		).toBe(false);

		const beforeFirst = observeTerminalBeforeInput(
			beginTerminalComposition(createTerminalInputState<string>()),
			'新',
			20,
			deduplicationWindow,
		);
		expect(beforeFirst).toMatchObject({
			publishText: true,
			state: {
				beforeInput: { text: '新', time: 20 },
				compositionTextPublished: true,
			},
		});
		expect(
			observeTerminalBeforeInput(beforeFirst.state, '新的', 20, deduplicationWindow).publishText,
		).toBe(false);
		const trailingEnd = finishTerminalComposition(beforeFirst.state, '新', 21, deduplicationWindow);
		expect(trailingEnd.publishText).toBe(false);
		expect(trailingEnd.state.beforeInput).toBeUndefined();
	});

	it('consumes key correlation on the first check at the exact time boundary', () => {
		const at99 = observeTerminalBeforeInput(
			recordTerminalKeyInput(createTerminalInputState<string>(), { text: 'a', time: 0 }),
			'a',
			99,
			deduplicationWindow,
		);
		expect(at99.duplicate).toBe(true);
		expect(at99.state.key).toBeUndefined();
		expect(observeTerminalBeforeInput(at99.state, 'a', 99, deduplicationWindow).duplicate).toBe(
			false,
		);

		const at100 = observeTerminalBeforeInput(
			recordTerminalKeyInput(createTerminalInputState<string>(), { text: 'a', time: 0 }),
			'a',
			100,
			deduplicationWindow,
		);
		expect(at100.duplicate).toBe(false);
		expect(at100.state.key).toBeUndefined();
	});

	it('consumes only matching composition, beforeinput, and cross-source paste correlations', () => {
		const composition = recordTerminalComposition(createTerminalInputState<string>(), {
			text: 'x',
			time: 0,
		});
		const mismatch = observeTerminalBeforeInput(composition, 'y', 1, deduplicationWindow);
		expect(mismatch.duplicate).toBe(false);
		expect(mismatch.state.composition).toEqual({ text: 'x', time: 0 });
		const match = observeTerminalBeforeInput(mismatch.state, 'x', 2, deduplicationWindow);
		expect(match.duplicate).toBe(true);
		expect(match.state.composition).toBeUndefined();

		const beforeInput = recordTerminalBeforeInput(
			beginTerminalComposition(createTerminalInputState<string>()),
			{ text: 'x', time: 0 },
		);
		expect(
			finishTerminalComposition(beforeInput, 'y', 1, deduplicationWindow).state.beforeInput,
		).toEqual({ text: 'x', time: 0 });
		expect(
			finishTerminalComposition(beforeInput, 'x', 99, deduplicationWindow).state.beforeInput,
		).toBeUndefined();
		const expiredBeforeInput = finishTerminalComposition(
			beforeInput,
			'x',
			100,
			deduplicationWindow,
		);
		expect(expiredBeforeInput.publishText).toBe(false);
		expect(expiredBeforeInput.state.beforeInput).toEqual({ text: 'x', time: 0 });

		const paste = recordTerminalPaste(createTerminalInputState<string>(), {
			source: 'clipboard',
			text: 'paste',
			time: 0,
		});
		expect(observeTerminalPaste(paste, 'paste', 'clipboard', 1, deduplicationWindow)).toEqual({
			duplicate: false,
			state: paste,
		});
		const pasteDuplicate = observeTerminalPaste(
			paste,
			'paste',
			'beforeinput',
			99,
			deduplicationWindow,
		);
		expect(pasteDuplicate.duplicate).toBe(true);
		expect(pasteDuplicate.state.paste).toBeUndefined();
		expect(
			observeTerminalPaste(pasteDuplicate.state, 'paste', 'beforeinput', 99, deduplicationWindow)
				.duplicate,
		).toBe(false);
	});

	it('clears every correlation while returning the one mouse repair sample', () => {
		let state = beginTerminalComposition(createTerminalInputState<string>());
		state = recordTerminalKeyInput(state, { text: 'key', time: 1 });
		state = recordTerminalComposition(state, { text: 'composition', time: 2 });
		state = recordTerminalBeforeInput(state, { text: 'beforeinput', time: 3 });
		state = recordTerminalPaste(state, {
			source: 'clipboard',
			text: 'paste',
			time: 4,
		});
		state = recordTerminalMouseSample(state, 'mouse');

		const repair = repairTerminalInputState(state);
		expect(repair.mouse).toBe('mouse');
		expect(repair.state).toEqual({
			beforeInput: undefined,
			composition: undefined,
			compositionPhase: 'cancelled-awaiting-end',
			compositionSettlement: {
				beforeInputObserved: false,
				kind: 'awaiting-native-end',
			},
			compositionTextPublished: false,
			key: undefined,
			mouse: undefined,
			paste: undefined,
		});

		const idle = finishTerminalComposition(repair.state, '', 5, deduplicationWindow).state;
		expect(observeTerminalBeforeInput(idle, 'key', 6, deduplicationWindow).duplicate).toBe(false);
		expect(observeTerminalBeforeInput(idle, 'composition', 6, deduplicationWindow).duplicate).toBe(
			false,
		);
		expect(
			observeTerminalPaste(idle, 'paste', 'beforeinput', 6, deduplicationWindow).duplicate,
		).toBe(false);
		expect(
			finishTerminalComposition(
				beginTerminalComposition(idle),
				'beforeinput',
				6,
				deduplicationWindow,
			).publishText,
		).toBe(true);
		expect(
			repairTerminalInputState(recordTerminalMouseSample(idle, undefined)).mouse,
		).toBeUndefined();
		expect(disposeTerminalInputState(state).state).toMatchObject({
			compositionPhase: 'idle',
		});
	});
});
