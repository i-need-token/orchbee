import { describe, expect, expectTypeOf, it } from 'vitest';

import { ActionRuntime, defineAction, noActionInput } from '../action-runtime';
import { createTypedActionRuntime } from '../createTypedActionRuntime';
import type { ActionExecution, ActionSignature } from '../action-runtime-types';

interface TestActionCatalog {
	readonly 'test.message.echo': ActionSignature<{ readonly message: string }, string>;
	readonly 'test.value.number': ActionSignature<void, number>;
	readonly 'test.value.text': ActionSignature<void, string>;
	readonly 'test.workspace.refresh': ActionSignature<void>;
}

describe('createTypedActionRuntime', () => {
	it('preserves the catalog input and output relationship for trusted callers', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		const typedRuntime = createTypedActionRuntime<TestActionCatalog>(runtime);
		typedRuntime.register(
			scope,
			defineAction<{ readonly message: string }, string, 'test.message.echo'>({
				category: { id: 'test', title: { id: 'test.category' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: (input) => input.message,
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (_context, input) => ({ kind: 'completed', value: input.message }),
				},
				id: 'test.message.echo',
				input: {
					name: 'message',
					parse: (input) =>
						typeof input === 'object' &&
						input !== null &&
						'message' in input &&
						typeof input.message === 'string'
							? { success: true, data: { message: input.message } }
							: { success: false, code: 'expected-message' },
				},
				title: { id: 'test.message.echo.title' },
			}),
		);
		typedRuntime.register(
			scope,
			defineAction<void, void, 'test.workspace.refresh'>({
				category: { id: 'test', title: { id: 'test.category' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'refresh',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: { execute: () => ({ kind: 'completed' }) },
				id: 'test.workspace.refresh',
				input: noActionInput,
				title: { id: 'test.workspace.refresh.title' },
			}),
		);
		const execution = typedRuntime.execute(
			'test.message.echo',
			{ message: 'hello' },
			{ scope, source: 'programmatic' },
		);

		expectTypeOf(execution).toEqualTypeOf<ActionExecution<string>>();
		await expect(execution.completion).resolves.toEqual({
			kind: 'completed',
			value: 'hello',
		});
		expect(
			typedRuntime.resolveAction('test.workspace.refresh', undefined, {
				scope,
				source: 'keyboard',
			}),
		).toMatchObject({ availability: { kind: 'enabled' } });

		const wrongOutput = defineAction<{ readonly message: string }, number, 'test.message.echo'>({
			category: { id: 'test', title: { id: 'test.category' } },
			execution: {
				concurrency: 'parallel',
				concurrencyKey: (input) => input.message,
				mode: 'foreground',
			},
			getAvailability: () => ({ kind: 'enabled' }),
			handler: { execute: () => ({ kind: 'completed', value: 1 }) },
			id: 'test.message.echo',
			input: {
				name: 'message',
				parse: (input) =>
					typeof input === 'object' &&
					input !== null &&
					'message' in input &&
					typeof input.message === 'string'
						? { success: true, data: { message: input.message } }
						: { success: false, code: 'expected-message' },
			},
			title: { id: 'test.message.echo.title' },
		});
		expect(() => {
			// @ts-expect-error -- verifies catalog output mismatches are rejected
			typedRuntime.register(scope, wrongOutput);
		}).toThrow(/already registered/);

		const wrongInput = defineAction<{ readonly message: string }, void, 'test.workspace.refresh'>({
			category: { id: 'test', title: { id: 'test.category' } },
			execution: {
				concurrency: 'parallel',
				concurrencyKey: (input) => input.message,
				mode: 'foreground',
			},
			getAvailability: () => ({ kind: 'enabled' }),
			handler: { execute: () => ({ kind: 'completed' }) },
			id: 'test.workspace.refresh',
			input: {
				name: 'message',
				parse: (input) =>
					typeof input === 'object' &&
					input !== null &&
					'message' in input &&
					typeof input.message === 'string'
						? { success: true, data: { message: input.message } }
						: { success: false, code: 'expected-message' },
			},
			title: { id: 'test.workspace.refresh.title' },
		});
		expect(() => {
			// @ts-expect-error -- verifies catalog input mismatches are rejected
			typedRuntime.register(scope, wrongInput);
		}).toThrow(/already registered/);

		const unionActionId: 'test.value.number' | 'test.value.text' =
			Math.random() > 0.5 ? 'test.value.number' : 'test.value.text';
		const unionOutput = defineAction<
			void,
			number | string,
			'test.value.number' | 'test.value.text'
		>({
			category: { id: 'test', title: { id: 'test.category' } },
			execution: {
				concurrency: 'parallel',
				concurrencyKey: () => 'union-output',
				mode: 'foreground',
			},
			getAvailability: () => ({ kind: 'enabled' }),
			handler: { execute: () => ({ kind: 'completed', value: 42 }) },
			id: unionActionId,
			input: noActionInput,
			title: { id: 'test.value.union.title' },
		});
		const registerUnionOutput = () => {
			// @ts-expect-error -- verifies union IDs cannot merge catalog output types
			typedRuntime.register(scope, unionOutput);
		};
		expect(registerUnionOutput).toBeTypeOf('function');
	});

	it('does not project catalog types onto registrations owned by another facade', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			defineAction<void, number, 'test.message.echo'>({
				category: { id: 'test', title: { id: 'test.category' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'alien',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: { execute: () => ({ kind: 'completed', value: 42 }) },
				id: 'test.message.echo',
				input: noActionInput,
				title: { id: 'test.message.echo.title' },
			}),
		);
		const typedRuntime = createTypedActionRuntime<TestActionCatalog>(runtime);
		const execution = typedRuntime.execute(
			'test.message.echo',
			{ message: 'not accepted by the alien registration' },
			{ scope, source: 'programmatic' },
		);

		expectTypeOf(execution).toEqualTypeOf<ActionExecution<string>>();
		await expect(execution.completion).resolves.toEqual({
			code: 'catalog-registration-mismatch',
			kind: 'blocked',
		});
	});
});
