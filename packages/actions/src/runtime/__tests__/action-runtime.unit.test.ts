import { describe, expect, it, vi } from 'vitest';

import { ActionRuntime, defineAction, noActionInput } from '../action-runtime';
import type {
	ActionAvailability,
	ActionDefinition,
	ActionExecutionContext,
	ActionResult,
} from '../action-runtime-types';

interface Deferred<Value> {
	readonly promise: Promise<Value>;
	reject(reason: unknown): void;
	resolve(value: Value): void;
}

function deferred<Value>(): Deferred<Value> {
	let rejectPromise: ((reason: unknown) => void) | undefined;
	let resolvePromise: ((value: Value) => void) | undefined;
	const promise = new Promise<Value>((resolve, reject) => {
		rejectPromise = reject;
		resolvePromise = resolve;
	});
	return {
		promise,
		reject: (reason) => rejectPromise?.(reason),
		resolve: (value) => resolvePromise?.(value),
	};
}

function action(
	id: string,
	execute: (
		context: ActionExecutionContext,
	) => ActionResult<string> | Promise<ActionResult<string>>,
	options: {
		readonly availability?: (() => ActionAvailability) | undefined;
		readonly concurrency?: 'parallel' | 'reject-duplicate' | 'replace-latest' | 'serialize';
		readonly confirmation?: boolean;
	} = {},
): ActionDefinition<void, string> {
	return defineAction({
		category: {
			id: 'test',
			title: { id: 'action.category.test' },
		},
		confirmation:
			options.confirmation === true
				? () => ({
						confirmLabel: { id: 'action.confirm' },
						description: { id: 'action.description' },
						risk: 'routine',
						title: { id: 'action.title' },
					})
				: undefined,
		execution: {
			concurrency: options.concurrency ?? 'parallel',
			concurrencyKey: () => 'same-target',
			mode: 'foreground',
		},
		getAvailability: options.availability ?? (() => ({ kind: 'enabled' })),
		handler: { execute },
		id,
		input: noActionInput,
		title: { id: `${id}.title` },
	});
}

describe('ActionRuntime', () => {
	it('rejects a foreign scope before allocating a managed execution', () => {
		const runtime = new ActionRuntime();
		const ownScope = runtime.createScope('own');
		const foreignRuntime = new ActionRuntime();
		const foreignScope = foreignRuntime.createScope('foreign');

		expect(() =>
			runtime.execute('test.missing.run', undefined, {
				scope: foreignScope,
				source: 'programmatic',
			}),
		).toThrow(/belongs to another runtime/);
		const execution = runtime.execute('test.missing.run', undefined, {
			scope: ownScope,
			source: 'programmatic',
		});

		expect(execution.executionId).toBe('action-execution-1');
		runtime.dispose();
		foreignRuntime.dispose();
	});

	it('rejects duplicate registrations in one scope and resolves the narrowest scope', async () => {
		const runtime = new ActionRuntime({ createExecutionId: () => 'execution' });
		const application = runtime.createScope('application');
		const panel = runtime.createScope('panel-1', application);
		const applicationAction = action('test.resource.run', () => ({
			kind: 'completed',
			value: 'application',
		}));
		runtime.register(application, applicationAction);
		expect(() => runtime.register(application, applicationAction)).toThrow(/already registered/);
		const panelRegistration = runtime.register(
			panel,
			action('test.resource.run', () => ({ kind: 'completed', value: 'panel' })),
		);
		const prepared = runtime
			.discover({ scope: panel, source: 'command-surface' })
			.find((result) => result.descriptor.id === 'test.resource.run');
		if (prepared?.kind !== 'ready') {
			throw new Error('Expected the panel action to be prepared');
		}

		const execution = runtime.execute('test.resource.run', undefined, {
			scope: panel,
			source: 'button',
		});

		expect(execution.executionId).toBe('execution');
		expect(execution.getSnapshot()).toMatchObject({
			result: { kind: 'completed', value: 'panel' },
			state: 'completed',
		});
		await expect(execution.completion).resolves.toEqual({
			kind: 'completed',
			value: 'panel',
		});

		panelRegistration.dispose();
		runtime.register(
			panel,
			action('test.resource.run', () => ({ kind: 'completed', value: 'replacement' })),
		);
		await expect(runtime.executePrepared(prepared.invocation).completion).resolves.toEqual({
			code: 'stale-prepared-invocation',
			kind: 'blocked',
		});
		await expect(
			runtime.executePrepared({
				actionId: prepared.invocation.actionId,
				invocation: prepared.invocation.invocation,
			}).completion,
		).resolves.toEqual({
			code: 'invalid-prepared-invocation',
			kind: 'blocked',
		});
	});

	it('enumerates visible actions, preserves disabled reasons, and hides denied automation', () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action('test.visible.run', () => ({ kind: 'completed' }), {
				availability: () => ({
					kind: 'disabled',
					reason: { id: 'action.disabled' },
				}),
			}),
		);
		runtime.register(
			scope,
			action('test.hidden.run', () => ({ kind: 'completed' }), {
				availability: () => ({ kind: 'hidden' }),
			}),
		);

		expect(runtime.list({ scope, source: 'command-surface' })).toEqual([
			expect.objectContaining({
				availability: {
					kind: 'disabled',
					reason: { id: 'action.disabled' },
				},
				descriptor: expect.objectContaining({ id: 'test.visible.run' }),
			}),
		]);
		expect(runtime.list({ scope, source: 'automation' })).toEqual([]);
		expect(
			runtime.resolveAction('test.visible.run', undefined, {
				scope,
				source: 'keyboard',
			}),
		).toMatchObject({
			availability: { kind: 'disabled' },
			descriptor: { id: 'test.visible.run' },
		});
		expect(
			runtime.execute('test.visible.run', undefined, { scope, source: 'automation' }).getSnapshot(),
		).toMatchObject({ result: { kind: 'unsupported' } });
	});

	it('discovers descendant scopes and preserves actions that still require input', async () => {
		const runtime = new ActionRuntime();
		const application = runtime.createScope('application');
		const leftPanel = runtime.createScope('left-panel', application);
		const rightPanel = runtime.createScope('right-panel', application);
		runtime.register(
			leftPanel,
			action('test.left.run', () => ({ kind: 'completed', value: 'left' })),
		);
		runtime.register(
			rightPanel,
			defineAction<{ readonly name: string }, string>({
				category: { id: 'test', title: { id: 'action.category.test' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: (input) => input.name,
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				getDiscoveryAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (_context, input) => ({ kind: 'completed', value: input.name }),
				},
				id: 'test.right.rename',
				input: {
					name: 'panel-name',
					parse: (input) =>
						typeof input === 'object' &&
						input !== null &&
						'name' in input &&
						typeof input.name === 'string'
							? { success: true, data: { name: input.name } }
							: { success: false, code: 'expected-panel-name' },
				},
				title: { id: 'test.right.rename.title' },
			}),
		);

		expect(runtime.discover({ scope: application, source: 'command-surface' })).toEqual([]);
		const unbound = runtime.discover(
			{ scope: application, source: 'command-surface' },
			{ includeDescendants: true },
		);
		expect(unbound).toEqual([
			expect.objectContaining({
				descriptor: expect.objectContaining({ id: 'test.left.run' }),
				kind: 'ready',
				scope: leftPanel,
			}),
			expect.objectContaining({
				invocation: expect.objectContaining({
					actionId: 'test.right.rename',
					inputName: 'panel-name',
				}),
				kind: 'requires-input',
				scope: rightPanel,
			}),
		]);

		const unboundRename = unbound.find((result) => result.descriptor.id === 'test.right.rename');
		if (unboundRename?.kind !== 'requires-input') {
			throw new Error('Expected an unbound descendant action');
		}
		expect(
			runtime.prepareInvocation(
				{
					actionId: unboundRename.invocation.actionId,
					inputName: unboundRename.invocation.inputName,
					invocation: unboundRename.invocation.invocation,
				},
				{ name: 'copied-token' },
			),
		).toBeNull();
		const candidateInput = { name: 'renamed' };
		const prepared = runtime.prepareInvocation(unboundRename.invocation, candidateInput);
		if (prepared === null) {
			throw new Error('Expected the descendant input to prepare successfully');
		}
		candidateInput.name = 'mutated-after-preparation';
		expect(prepared.invocation.invocation.scope).toBe(rightPanel);
		await expect(runtime.executePrepared(prepared.invocation).completion).resolves.toEqual({
			kind: 'completed',
			value: 'renamed',
		});
	});

	it('validates dynamic input and maps unexpected handler failures to a terminal result', async () => {
		const onUnexpectedError = vi.fn<(actionId: string, error: unknown) => void>();
		const runtime = new ActionRuntime({ onUnexpectedError });
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			defineAction<string, never>({
				category: { id: 'test', title: { id: 'action.category.test' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: (input) => input,
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: () => {
						throw new Error('private failure');
					},
				},
				id: 'test.input.run',
				input: {
					name: 'string',
					parse: (input) =>
						typeof input === 'string'
							? { success: true, data: input }
							: { success: false, code: 'expected-string' },
				},
				title: { id: 'test.input.run.title' },
			}),
		);

		const invalid = runtime.execute('test.input.run', 1, { scope, source: 'programmatic' });
		expect(invalid.getSnapshot()).toMatchObject({
			result: {
				code: 'invalid-action-input',
				details: { validationCode: 'expected-string' },
				kind: 'blocked',
			},
		});

		const failed = runtime.execute('test.input.run', 'valid', {
			scope,
			source: 'programmatic',
		});
		await expect(failed.completion).resolves.toEqual({
			failure: { code: 'unexpected-error' },
			kind: 'failed',
		});
		expect(onUnexpectedError).toHaveBeenCalledWith('test.input.run', expect.any(Error));
	});

	it('rechecks availability after asynchronous confirmation before running a handler', async () => {
		const confirmation = deferred<boolean>();
		let enabled = true;
		const handler = vi.fn<() => ActionResult<string>>(() => ({ kind: 'completed' }));
		const runtime = new ActionRuntime({ confirm: () => confirmation.promise });
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action('test.confirm.run', handler, {
				availability: () =>
					enabled ? { kind: 'enabled' } : { kind: 'disabled', reason: { id: 'became-disabled' } },
				confirmation: true,
			}),
		);

		const execution = runtime.execute('test.confirm.run', undefined, {
			scope,
			source: 'context-menu',
		});
		expect(execution.getSnapshot().state).toBe('awaiting-confirmation');
		enabled = false;
		confirmation.resolve(true);

		await expect(execution.completion).resolves.toMatchObject({
			code: 'action-unavailable',
			kind: 'blocked',
		});
		expect(handler).not.toHaveBeenCalled();
	});

	it('keeps the invocation target stable through asynchronous confirmation', async () => {
		const confirmation = deferred<boolean>();
		const confirmedTargets: string[] = [];
		const executedTargets: string[] = [];
		const runtime = new ActionRuntime({
			confirm: (_confirmation, invocation) => {
				confirmedTargets.push(invocation.target?.id ?? 'missing');
				return confirmation.promise;
			},
		});
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action(
				'test.confirmed-target.run',
				(context) => {
					executedTargets.push(context.target?.id ?? 'missing');
					return { kind: 'completed' };
				},
				{ confirmation: true },
			),
		);
		const target = { id: 'before-confirmation', type: 'resource' };
		const execution = runtime.execute('test.confirmed-target.run', undefined, {
			scope,
			source: 'button',
			target,
		});

		target.id = 'after-confirmation';
		confirmation.resolve(true);

		await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
		expect(confirmedTargets).toEqual(['before-confirmation']);
		expect(executedTargets).toEqual(['before-confirmation']);
	});

	it('releases a serialized queue when an awaiting confirmation is cancelled', async () => {
		const firstConfirmation = deferred<boolean>();
		let confirmationCount = 0;
		const runtime = new ActionRuntime({
			confirm: () => {
				confirmationCount += 1;
				return confirmationCount === 1 ? firstConfirmation.promise : true;
			},
		});
		const scope = runtime.createScope('application');
		const handler = vi.fn<() => ActionResult<string>>(() => ({ kind: 'completed' }));
		runtime.register(
			scope,
			action('test.confirm-queue.run', handler, {
				concurrency: 'serialize',
				confirmation: true,
			}),
		);

		const first = runtime.execute('test.confirm-queue.run', undefined, {
			scope,
			source: 'button',
		});
		const second = runtime.execute('test.confirm-queue.run', undefined, {
			scope,
			source: 'button',
		});
		expect(first.getSnapshot().state).toBe('awaiting-confirmation');
		expect(second.getSnapshot().state).toBe('resolving');

		first.cancel();

		await expect(first.completion).resolves.toEqual({ kind: 'cancelled' });
		await expect(second.completion).resolves.toEqual({ kind: 'completed' });
		expect(handler).toHaveBeenCalledOnce();
		firstConfirmation.resolve(false);
	});

	it('does not present confirmation after an awaiting-confirmation observer cancels', async () => {
		const firstCompletion = deferred<ActionResult<string>>();
		const confirm = vi.fn<() => boolean>(() => true);
		let handlerCallCount = 0;
		const handler = vi.fn<() => ActionResult<string> | Promise<ActionResult<string>>>(() => {
			handlerCallCount += 1;
			return handlerCallCount === 1
				? firstCompletion.promise
				: { kind: 'completed', value: 'successor' };
		});
		const runtime = new ActionRuntime({ confirm });
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action('test.confirm-observer-cancel.run', handler, {
				concurrency: 'serialize',
				confirmation: true,
			}),
		);

		const first = runtime.execute('test.confirm-observer-cancel.run', undefined, {
			scope,
			source: 'button',
		});
		const cancelled = runtime.execute('test.confirm-observer-cancel.run', undefined, {
			scope,
			source: 'button',
		});
		cancelled.subscribe(() => {
			if (cancelled.getSnapshot().state === 'awaiting-confirmation') {
				cancelled.cancel();
			}
		});
		const successor = runtime.execute('test.confirm-observer-cancel.run', undefined, {
			scope,
			source: 'button',
		});

		firstCompletion.resolve({ kind: 'completed', value: 'first' });

		await expect(first.completion).resolves.toEqual({ kind: 'completed', value: 'first' });
		await expect(cancelled.completion).resolves.toEqual({ kind: 'cancelled' });
		await expect(successor.completion).resolves.toEqual({
			kind: 'completed',
			value: 'successor',
		});
		expect(confirm).toHaveBeenCalledTimes(2);
		expect(handler).toHaveBeenCalledTimes(2);
	});

	it('does not invoke a handler after a running observer cancels', async () => {
		const firstCompletion = deferred<ActionResult<string>>();
		let handlerCallCount = 0;
		const handler = vi.fn<() => ActionResult<string> | Promise<ActionResult<string>>>(() => {
			handlerCallCount += 1;
			return handlerCallCount === 1
				? firstCompletion.promise
				: { kind: 'completed', value: 'successor' };
		});
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action('test.running-observer-cancel.run', handler, {
				concurrency: 'serialize',
			}),
		);

		const first = runtime.execute('test.running-observer-cancel.run', undefined, {
			scope,
			source: 'button',
		});
		const cancelled = runtime.execute('test.running-observer-cancel.run', undefined, {
			scope,
			source: 'button',
		});
		cancelled.subscribe(() => {
			if (cancelled.getSnapshot().state === 'running') {
				cancelled.cancel();
			}
		});
		const successor = runtime.execute('test.running-observer-cancel.run', undefined, {
			scope,
			source: 'button',
		});

		firstCompletion.resolve({ kind: 'completed', value: 'first' });

		await expect(first.completion).resolves.toEqual({ kind: 'completed', value: 'first' });
		await expect(cancelled.completion).resolves.toEqual({ kind: 'cancelled' });
		await expect(successor.completion).resolves.toEqual({
			kind: 'completed',
			value: 'successor',
		});
		expect(handler).toHaveBeenCalledTimes(2);
	});

	it('resolves a confirmation presenter from the narrowest active scope', async () => {
		const fallbackConfirm = vi.fn<() => boolean>(() => false);
		const scopedConfirm = vi.fn<() => boolean>(() => true);
		const runtime = new ActionRuntime({ confirm: fallbackConfirm });
		const application = runtime.createScope('application');
		const panel = runtime.createScope('panel', application);
		runtime.register(
			application,
			action('test.scoped-confirm.run', () => ({ kind: 'completed' }), {
				confirmation: true,
			}),
		);
		const presenter = runtime.registerConfirmationPresenter(panel, scopedConfirm);

		await expect(
			runtime.execute('test.scoped-confirm.run', undefined, {
				scope: panel,
				source: 'context-menu',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(scopedConfirm).toHaveBeenCalledOnce();
		expect(fallbackConfirm).not.toHaveBeenCalled();

		presenter.dispose();
		await expect(
			runtime.execute('test.scoped-confirm.run', undefined, {
				scope: panel,
				source: 'context-menu',
			}).completion,
		).resolves.toEqual({ kind: 'cancelled' });
		expect(fallbackConfirm).toHaveBeenCalledOnce();
	});

	it('contains discovery, concurrency-key, and confirmation failures', async () => {
		const onUnexpectedError = vi.fn<(actionId: string, error: unknown) => void>();
		const runtime = new ActionRuntime({ onUnexpectedError });
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			defineAction<void, never>({
				category: { id: 'test', title: { id: 'action.category.test' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'never',
					mode: 'foreground',
				},
				getAvailability: () => {
					throw new Error('availability failure');
				},
				handler: {
					execute: () => ({ kind: 'completed' }),
				},
				id: 'test.discovery-failure.run',
				input: noActionInput,
				title: { id: 'test.discovery-failure.run.title' },
			}),
		);
		runtime.register(
			scope,
			defineAction<void, never>({
				category: { id: 'test', title: { id: 'action.category.test' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => {
						throw new Error('key failure');
					},
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: () => ({ kind: 'completed' }),
				},
				id: 'test.key-failure.run',
				input: noActionInput,
				title: { id: 'test.key-failure.run.title' },
			}),
		);
		runtime.register(
			scope,
			defineAction<void, never>({
				category: { id: 'test', title: { id: 'action.category.test' } },
				confirmation: () => {
					throw new Error('confirmation failure');
				},
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'confirmation',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: () => ({ kind: 'completed' }),
				},
				id: 'test.confirmation-failure.run',
				input: noActionInput,
				title: { id: 'test.confirmation-failure.run.title' },
			}),
		);

		expect(runtime.list({ scope, source: 'command-surface' })).toHaveLength(2);
		expect(
			runtime.resolveAction('test.discovery-failure.run', undefined, {
				scope,
				source: 'command-surface',
			}),
		).toBeNull();
		await expect(
			runtime.execute('test.key-failure.run', undefined, {
				scope,
				source: 'programmatic',
			}).completion,
		).resolves.toEqual({
			failure: { code: 'unexpected-error' },
			kind: 'failed',
		});
		await expect(
			runtime.execute('test.confirmation-failure.run', undefined, {
				scope,
				source: 'programmatic',
			}).completion,
		).resolves.toEqual({
			failure: { code: 'unexpected-error' },
			kind: 'failed',
		});
		expect(onUnexpectedError).toHaveBeenCalledTimes(4);
	});

	it('maps a rejected asynchronous confirmation to a failed terminal result', async () => {
		const onUnexpectedError = vi.fn<(actionId: string, error: unknown) => void>();
		const runtime = new ActionRuntime({
			confirm: () => Promise.reject(new Error('presenter failure')),
			onUnexpectedError,
		});
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action('test.confirm-rejection.run', () => ({ kind: 'completed' }), {
				confirmation: true,
			}),
		);

		await expect(
			runtime.execute('test.confirm-rejection.run', undefined, {
				scope,
				source: 'context-menu',
			}).completion,
		).resolves.toEqual({
			failure: { code: 'unexpected-error' },
			kind: 'failed',
		});
		expect(onUnexpectedError).toHaveBeenCalledWith('test.confirm-rejection.run', expect.any(Error));
	});

	it('keeps execution completion independent from a failing diagnostic sink', async () => {
		const runtime = new ActionRuntime({
			onUnexpectedError: () => {
				throw new Error('diagnostic sink failure');
			},
		});
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action('test.diagnostic-sink.run', () => {
				throw new Error('handler failure');
			}),
		);

		await expect(
			runtime.execute('test.diagnostic-sink.run', undefined, {
				scope,
				source: 'programmatic',
			}).completion,
		).resolves.toEqual({
			failure: { code: 'unexpected-error' },
			kind: 'failed',
		});
	});

	it('isolates execution observers and releases serialized work after completion', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		const pending: Deferred<ActionResult<string>>[] = [];
		runtime.register(
			scope,
			action(
				'test.observer.run',
				() => {
					const completion = deferred<ActionResult<string>>();
					pending.push(completion);
					return completion.promise;
				},
				{ concurrency: 'serialize' },
			),
		);
		const first = runtime.execute('test.observer.run', undefined, {
			scope,
			source: 'button',
		});
		const healthyListener = vi.fn<() => void>();
		first.subscribe(() => {
			throw new Error('observer failure');
		});
		first.subscribe(healthyListener);
		const second = runtime.execute('test.observer.run', undefined, {
			scope,
			source: 'button',
		});

		pending[0]?.resolve({ kind: 'completed', value: 'first' });

		await expect(first.completion).resolves.toEqual({
			kind: 'completed',
			value: 'first',
		});
		expect(healthyListener).toHaveBeenCalled();
		expect(second.getSnapshot().state).toBe('running');
		pending[1]?.resolve({ kind: 'completed', value: 'second' });
		await expect(second.completion).resolves.toEqual({
			kind: 'completed',
			value: 'second',
		});
	});

	it('isolates registry observers without leaking registrations', () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		const healthyListener = vi.fn<() => void>();
		runtime.subscribe(() => {
			throw new Error('observer failure');
		});
		runtime.subscribe(healthyListener);

		const registration = runtime.register(
			scope,
			action('test.registry-observer.run', () => ({ kind: 'completed' })),
		);

		expect(healthyListener).toHaveBeenCalledOnce();
		expect(runtime.list({ scope, source: 'command-surface' })).toHaveLength(1);
		registration.dispose();
		expect(healthyListener).toHaveBeenCalledTimes(2);
		expect(runtime.list({ scope, source: 'command-surface' })).toEqual([]);
	});

	it('publishes progress and lets a committed async handler complete after cancellation', async () => {
		const completion = deferred<ActionResult<string>>();
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action('test.async.run', (context) => {
				context.reportProgress({ phase: 'prepare', completed: 1, total: 2 });
				context.markCommitted();
				return completion.promise;
			}),
		);

		const execution = runtime.execute('test.async.run', undefined, {
			scope,
			source: 'button',
		});
		expect(execution.getSnapshot()).toMatchObject({
			progress: { phase: 'prepare', completed: 1, total: 2 },
			state: 'running',
		});
		execution.cancel();
		expect(execution.getSnapshot().state).toBe('cancelling');
		completion.resolve({ kind: 'completed', value: 'job-1' });

		await expect(execution.completion).resolves.toEqual({
			kind: 'completed',
			value: 'job-1',
		});
	});

	it('fails a handler that reports cancellation after its commit point', async () => {
		const onUnexpectedError = vi.fn<(actionId: string, error: unknown) => void>();
		const runtime = new ActionRuntime({ onUnexpectedError });
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action('test.invalid-commit.run', (context) => {
				context.markCommitted();
				return { kind: 'cancelled' };
			}),
		);

		await expect(
			runtime.execute('test.invalid-commit.run', undefined, {
				scope,
				source: 'button',
			}).completion,
		).resolves.toEqual({
			failure: { code: 'unexpected-error' },
			kind: 'failed',
		});
		expect(onUnexpectedError).toHaveBeenCalledWith(
			'test.invalid-commit.run',
			expect.objectContaining({
				message: 'An Action handler returned cancelled after crossing its commit point',
			}),
		);
	});

	it('cancels an uncommitted async handler even when it returns success late', async () => {
		const completion = deferred<ActionResult<string>>();
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action('test.cancel.run', () => completion.promise),
		);

		const execution = runtime.execute('test.cancel.run', undefined, {
			scope,
			source: 'button',
		});
		execution.cancel();
		completion.resolve({ kind: 'completed', value: 'too-late' });

		await expect(execution.completion).resolves.toEqual({ kind: 'cancelled' });
	});

	it('maps an uncommitted asynchronous rejection after cancellation to cancelled', async () => {
		const onUnexpectedError = vi.fn<(actionId: string, error: unknown) => void>();
		const runtime = new ActionRuntime({ onUnexpectedError });
		const scope = runtime.createScope('application');
		const completion = deferred<ActionResult<string>>();
		runtime.register(
			scope,
			action('test.cancel-rejection.run', () => completion.promise),
		);

		const execution = runtime.execute('test.cancel-rejection.run', undefined, {
			scope,
			source: 'button',
		});
		execution.cancel();
		completion.reject(new Error('aborted handler'));

		await expect(execution.completion).resolves.toEqual({ kind: 'cancelled' });
		expect(onUnexpectedError).not.toHaveBeenCalled();
	});

	it('enforces reject-duplicate, replace-latest, and serialize per semantic key', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		const rejectedFirst = deferred<ActionResult<string>>();
		runtime.register(
			scope,
			action('test.reject.run', () => rejectedFirst.promise, {
				concurrency: 'reject-duplicate',
			}),
		);
		const firstReject = runtime.execute('test.reject.run', undefined, {
			scope,
			source: 'button',
		});
		const secondReject = runtime.execute('test.reject.run', undefined, {
			scope,
			source: 'button',
		});
		expect(secondReject.getSnapshot()).toMatchObject({
			result: { code: 'duplicate-action-execution', kind: 'blocked' },
		});
		rejectedFirst.resolve({ kind: 'completed' });
		await firstReject.completion;

		const replaced = deferred<ActionResult<string>>();
		runtime.register(
			scope,
			action(
				'test.replace.run',
				(context) => {
					if (context.signal.aborted) return { kind: 'cancelled' };
					return replaced.promise;
				},
				{ concurrency: 'replace-latest' },
			),
		);
		const firstReplace = runtime.execute('test.replace.run', undefined, {
			scope,
			source: 'button',
		});
		const secondReplace = runtime.execute('test.replace.run', undefined, {
			scope,
			source: 'button',
		});
		expect(firstReplace.getSnapshot().state).toBe('cancelling');
		expect(secondReplace.getSnapshot().state).toBe('running');
		replaced.resolve({ kind: 'completed' });
		await expect(firstReplace.completion).resolves.toEqual({ kind: 'cancelled' });
		await expect(secondReplace.completion).resolves.toEqual({ kind: 'completed' });

		const serialized: Deferred<ActionResult<string>>[] = [];
		runtime.register(
			scope,
			action(
				'test.serialize.run',
				() => {
					const item = deferred<ActionResult<string>>();
					serialized.push(item);
					return item.promise;
				},
				{ concurrency: 'serialize' },
			),
		);
		const firstSerialized = runtime.execute('test.serialize.run', undefined, {
			scope,
			source: 'button',
		});
		const secondSerialized = runtime.execute('test.serialize.run', undefined, {
			scope,
			source: 'button',
		});
		expect(serialized).toHaveLength(1);
		expect(secondSerialized.getSnapshot().state).toBe('resolving');
		serialized[0]?.resolve({ kind: 'completed', value: 'first' });
		await firstSerialized.completion;
		expect(serialized).toHaveLength(2);
		expect(secondSerialized.getSnapshot().state).toBe('running');
		serialized[1]?.resolve({ kind: 'completed', value: 'second' });
		await expect(secondSerialized.completion).resolves.toEqual({
			kind: 'completed',
			value: 'second',
		});
	});

	it('drains a large serialized queue of cancelled executions without recursion', async () => {
		const firstCompletion = deferred<ActionResult<string>>();
		let handlerCallCount = 0;
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			action(
				'test.serialize-cancelled.run',
				() => {
					handlerCallCount += 1;
					return handlerCallCount === 1
						? firstCompletion.promise
						: { kind: 'completed', value: 'successor' };
				},
				{ concurrency: 'serialize' },
			),
		);
		const first = runtime.execute('test.serialize-cancelled.run', undefined, {
			scope,
			source: 'programmatic',
		});
		const cancelled = Array.from({ length: 25_000 }, () =>
			runtime.execute('test.serialize-cancelled.run', undefined, {
				scope,
				source: 'programmatic',
			}),
		);
		for (const execution of cancelled) execution.cancel();
		await Promise.all(cancelled.map((execution) => execution.completion));
		const successor = runtime.execute('test.serialize-cancelled.run', undefined, {
			scope,
			source: 'programmatic',
		});

		firstCompletion.resolve({ kind: 'completed', value: 'first' });

		await expect(first.completion).resolves.toEqual({ kind: 'completed', value: 'first' });
		await expect(successor.completion).resolves.toEqual({
			kind: 'completed',
			value: 'successor',
		});
		expect(handlerCallCount).toBe(2);
	});

	it('notifies on registration and disposal and returns unsupported for missing actions', () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('application');
		const listener = vi.fn<() => void>();
		const subscription = runtime.subscribe(listener);
		const registration = runtime.register(
			scope,
			action('test.notify.run', () => ({ kind: 'completed' })),
		);
		registration.dispose();

		expect(listener).toHaveBeenCalledTimes(2);
		expect(
			runtime.execute('test.missing.run', undefined, { scope, source: 'button' }).getSnapshot(),
		).toMatchObject({ result: { kind: 'unsupported' }, state: 'unsupported' });
		subscription.dispose();
	});
});
