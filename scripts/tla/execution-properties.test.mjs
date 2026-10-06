import assert from 'node:assert/strict';
import test from 'node:test';

import { HistoricalModelFailure, runModelChecks } from './model-checks.mjs';

function random(seed) {
	let value = seed >>> 0;
	return (limit) => {
		value = (Math.imul(value, 1664525) + 1013904223) >>> 0;
		return value % limit;
	};
}

function deferred() {
	let resolve;
	const promise = new Promise((done) => {
		resolve = done;
	});
	return { resolve, promise };
}

const tick = () => new Promise((resolve) => setImmediate(resolve));
const output = (actions) =>
	actions.map((action) => `<${action} line 1, col 1 of module Example>: 1:2\n`).join('');

function initial(seed) {
	const choose = random(seed);
	const mode = seed % 8;
	const models = ['alpha', 'beta'].map((id) => ({
		id,
		workers: 1 + choose(4),
		requiredCoverage: ['A', 'B'],
		variants: Array.from({ length: 1 + choose(3) }, (_, index) => ({ name: `v${index}` })),
	}));
	const cached = {};
	const partial = [];
	const previousAggregates = {};
	if ([0, 1, 2].includes(mode)) {
		for (const variant of models[0].variants)
			cached[`alpha/${variant.name}`] = {
				status: mode === 1 ? 'failed' : 'passed',
				actions: mode === 0 ? ['A'] : ['A', 'B'],
			};
		if (mode === 0) previousAggregates.alpha = 'failed';
	}
	if (mode === 3) partial.push(`alpha/${models[0].variants[0].name}`);
	if (mode === 6) {
		models[0].workers = 1;
		models[1].workers = 1;
	}
	return {
		seed,
		mode,
		models,
		slots: mode === 6 ? 3 : 1 + choose(4),
		processors: mode === 6 ? 4 : 1 + choose(4),
		cached,
		partial,
		previousAggregates,
	};
}

/** An event oracle: independently checks legal admission, terminal evidence and model-local action sets. */
async function replay(trace, generate = false) {
	assert.equal(trace.schemaVersion, 1);
	const state = structuredClone(trace.initial);
	const choose = random(state.seed + 97);
	const records = new Map(Object.entries(state.cached));
	const aggregates = new Map(Object.entries(state.previousAggregates));
	const events = [];
	const branches = new Set();
	const commands = trace.commands;
	let commandIndex = 0;
	for (let invocation = 0; invocation < 2; invocation += 1) {
		if (invocation) branches.add('scope-continuation');
		const selected = invocation === 0 && state.mode === 2 ? [state.models[0]] : state.models;
		const expectedOrder = selected.flatMap((model) =>
			model.variants.map((variant) => `${model.id}/${variant.name}`),
		);
		const active = new Map();
		const admitted = [];
		const skipped = [];
		let closed = false;
		let liveWeight = 0;
		let finished = false;
		const controller = new AbortController();
		const previousFailures = new Map(
			[...aggregates]
				.filter(([, status]) => status === 'failed')
				.map(([id]) => [id, new HistoricalModelFailure(`${id}: historical aggregate`)]),
		);
		const checking = runModelChecks(
			selected,
			state.slots,
			async (model, variant, fail) => {
				const id = `${model.id}/${variant.name}`;
				assert.equal(closed, false, 'admission-after-failure');
				assert.equal(id, expectedOrder[admitted.length], 'fifo-order');
				admitted.push(id);
				const cached = records.get(id);
				if (cached) {
					events.push(`reuse:${id}:${cached.status}`);
					branches.add(cached.status === 'passed' ? 'cached-pass' : 'historical-failure');
					if (cached.status === 'failed')
						throw new HistoricalModelFailure(`${id}: historical configuration`);
					return output(cached.actions);
				}
				branches.add('fresh');
				if (state.partial.includes(id)) branches.add('partial-record');
				assert.ok(active.size < state.slots, 'process-budget');
				assert.ok(
					active.size === 0 || liveWeight + model.workers <= state.processors,
					'worker-budget',
				);
				if (model.workers > state.processors) {
					assert.equal(active.size, 0, 'oversize-exclusive');
					branches.add('oversize');
				}
				if (active.size) branches.add('concurrent');
				liveWeight += model.workers;
				const completion = deferred();
				active.set(id, { model, completion, fail });
				events.push(`start:${id}`);
				const result = await completion.promise;
				active.delete(id);
				liveWeight -= model.workers;
				assert.equal(records.has(id), false, 'record-overwrite');
				records.set(id, result);
				events.push(`close:${id}:${result.status}`);
				if (result.status === 'failed') throw new Error(`${id}: current failure`);
				return output(result.actions);
			},
			(model, variant) => skipped.push(`${model.id}/${variant.name}`),
			{
				processors: state.processors,
				signal: controller.signal,
				previousModelFailures: previousFailures,
				onModelOutcome(model, outcome) {
					const configurations = model.variants.map((variant) =>
						records.get(`${model.id}/${variant.name}`),
					);
					assert.ok(configurations.every(Boolean), 'aggregate-before-terminal');
					const actions = new Set(
						configurations
							.filter((record) => record.status === 'passed')
							.flatMap((record) => record.actions),
					);
					const expected =
						configurations.every((record) => record.status === 'passed') &&
						model.requiredCoverage.every((action) => actions.has(action)) &&
						aggregates.get(model.id) !== 'failed'
							? 'passed'
							: 'failed';
					assert.equal(outcome.status, expected, 'model-proof');
					if (
						aggregates.get(model.id) === 'failed' &&
						configurations.every((record) => record.status === 'passed')
					)
						branches.add('aggregate-history');
					if (
						!previousFailures.has(model.id) &&
						configurations.every((record) => record.status === 'passed') &&
						model.requiredCoverage.some((action) => !actions.has(action))
					) {
						closed = true;
						branches.add('coverage-failure');
					}
					aggregates.set(model.id, outcome.status);
					events.push(`model:${model.id}:${outcome.status}`);
				},
			},
		)
			.then(
				() => ({ status: 'passed' }),
				(error) => ({ status: 'failed', error }),
			)
			.finally(() => {
				finished = true;
			});
		try {
			await tick();
			while (!finished) {
				let command;
				if (generate) {
					const ids = [...active.keys()];
					assert.ok(ids.length, 'no-progress');
					const id = ids[choose(ids.length)];
					const current = active.get(id);
					const failure = state.mode === 4 && !closed && !branches.has('current-failure');
					const cancellation = state.mode === 5 && !closed && !branches.has('cancel');
					let kind = 'close';
					if (failure) kind = 'fail';
					else if (cancellation) kind = 'cancel';
					command = {
						invocation,
						kind,
						id,
						status: closed || current.failed ? 'failed' : 'passed',
						actions: state.mode === 7 && id.startsWith('alpha/') ? ['A'] : ['A', 'B'],
					};
					commands.push(command);
				} else command = commands[commandIndex];
				if (!command || command.invocation !== invocation || !active.has(command.id))
					throw new Error('invalid-trace');
				commandIndex += 1;
				const current = active.get(command.id);
				if (command.kind === 'fail') {
					closed = true;
					current.failed = true;
					current.fail(new Error('current failure before log drain'));
					branches.add('current-failure');
					branches.add('delayed-drain');
				} else if (command.kind === 'cancel') {
					closed = true;
					controller.abort(new Error('cancelled'));
					branches.add('cancel');
				} else if (command.kind === 'close')
					current.completion.resolve({ status: command.status, actions: command.actions });
				else throw new Error('invalid-trace');
				await tick();
			}
			const result = await checking;
			assert.equal(active.size, 0, 'unclosed-child');
			assert.deepEqual([...admitted, ...skipped], expectedOrder, 'missing-outcome');
			if (closed || selected.some((model) => aggregates.get(model.id) === 'failed'))
				assert.equal(result.status, 'failed', 'false-success');
			else assert.equal(result.status, 'passed', 'false-failure');
			for (const [id, record] of Object.entries(state.cached))
				assert.deepEqual(records.get(id), record, 'cached-record-mutated');
		} catch (error) {
			controller.abort(new Error('oracle trace rejected'));
			for (const current of active.values())
				current.completion.resolve({ status: 'failed', actions: [] });
			await checking;
			throw error;
		}
	}
	if (commandIndex !== commands.length) throw new Error('invalid-trace');
	return {
		events,
		branches: [...branches].toSorted(),
		trace: { schemaVersion: 1, initial: state, commands },
	};
}

/** Preserve the same defect fingerprint; invalid dependent traces never count as reductions. */
async function shrink(trace, fingerprint, judge) {
	let current = structuredClone(trace);
	for (
		let width = Math.ceil(current.commands.length / 2);
		width >= 1;
		width = Math.floor(width / 2)
	) {
		for (let index = 0; index < current.commands.length; index += 1) {
			const candidate = {
				...current,
				commands: current.commands.filter(
					(_, position) => position < index || position >= index + width,
				),
			};
			if ((await judge(candidate)) === fingerprint) {
				current = candidate;
				index = -1;
			}
		}
	}
	for (const field of ['slots', 'processors']) {
		const candidate = structuredClone(current);
		candidate.initial[field] = 1;
		if ((await judge(candidate)) === fingerprint) current = candidate;
	}
	for (let index = 0; index < current.initial.models.length; index += 1) {
		const candidate = structuredClone(current);
		candidate.initial.models[index].workers = 1;
		if ((await judge(candidate)) === fingerprint) current = candidate;
		const fewer = structuredClone(current);
		fewer.initial.models[index].variants = fewer.initial.models[index].variants.slice(0, 1);
		if ((await judge(fewer)) === fingerprint) current = fewer;
	}
	return current;
}

test('64 seeded stateful traces replay deterministically and reach every required execution class', async () => {
	const reached = new Set();
	for (let seed = 1; seed <= 64; seed += 1) {
		const source = { schemaVersion: 1, initial: initial(seed), commands: [] };
		let generated;
		try {
			generated = await replay(source, true);
			assert.ok(generated.trace.commands.length <= 128);
			const repeated = await replay(JSON.parse(JSON.stringify(generated.trace)));
			assert.deepEqual(repeated.events, generated.events, `seed ${seed}`);
			assert.deepEqual(repeated.branches, generated.branches);
		} catch (error) {
			const fingerprint = error.message;
			const reduced = await shrink(generated?.trace ?? source, fingerprint, async (candidate) => {
				try {
					await replay(candidate);
					return undefined;
				} catch (failure) {
					return failure.message;
				}
			});
			throw new Error(`Stateful trace failed: ${JSON.stringify(reduced)}`, { cause: error });
		}
		for (const branch of generated.branches) reached.add(branch);
	}
	for (const branch of [
		'fresh',
		'cached-pass',
		'historical-failure',
		'partial-record',
		'aggregate-history',
		'concurrent',
		'oversize',
		'current-failure',
		'cancel',
		'delayed-drain',
		'coverage-failure',
		'scope-continuation',
	])
		assert.ok(reached.has(branch), `unreached generated branch: ${branch}`);
});

test('shrinking removes irrelevant actions and simplifies capacities while retaining the defect', async () => {
	const trace = {
		schemaVersion: 1,
		initial: initial(8),
		commands: [{ kind: 'noise' }, { kind: 'defect' }, { kind: 'noise' }],
	};
	const reduced = await shrink(trace, 'witness', async (candidate) =>
		candidate.commands.some((command) => command.kind === 'defect') ? 'witness' : undefined,
	);
	assert.deepEqual(reduced.commands, [{ kind: 'defect' }]);
	assert.equal(reduced.initial.slots, 1);
	assert.equal(reduced.initial.processors, 1);
	assert.ok(
		reduced.initial.models.every((model) => model.workers === 1 && model.variants.length === 1),
	);
});
