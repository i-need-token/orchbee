// @vitest-environment jsdom

import { describe, expect, it, vi } from 'vitest';

import { ActionRuntime, defineAction, noActionInput } from '../action-runtime';
import type { ActionResult } from '../action-runtime-types';

interface Deferred<Value> {
	readonly promise: Promise<Value>;
	resolve(value: Value): void;
}

function deferred<Value>(): Deferred<Value> {
	let resolvePromise: ((value: Value) => void) | undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return {
		promise,
		resolve: (value) => resolvePromise?.(value),
	};
}

function isPromise(value: unknown): value is Promise<unknown> {
	return (
		value !== null &&
		(typeof value === 'object' || typeof value === 'function') &&
		'then' in value &&
		typeof value.then === 'function'
	);
}

function crossRealmPromise<Value>(source: Promise<Value>): Promise<Value> {
	const frame = document.createElement('iframe');
	document.body.append(frame);
	const foreignPromise: unknown =
		frame.contentWindow === null ? undefined : Reflect.get(frame.contentWindow, 'Promise');
	if (typeof foreignPromise !== 'function' || foreignPromise === Promise) {
		frame.remove();
		throw new Error('Expected the iframe to provide a distinct Promise implementation');
	}
	const resolve: unknown = Reflect.get(foreignPromise, 'resolve');
	if (typeof resolve !== 'function') {
		frame.remove();
		throw new Error('Expected the iframe Promise implementation to expose resolve');
	}
	const promise: unknown = Reflect.apply(resolve, foreignPromise, [source]);
	frame.remove();
	if (!isPromise(promise)) {
		throw new Error('Expected the iframe Promise implementation to return a Promise');
	}
	return promise.then(() => source);
}

describe('ActionRuntime cross-realm execution', () => {
	it('awaits iframe Promises from confirmation and handlers', async () => {
		const confirmation = deferred<boolean>();
		const handlerCompletion = deferred<ActionResult<string>>();
		const handler = vi.fn<() => Promise<ActionResult<string>>>(() =>
			crossRealmPromise(handlerCompletion.promise),
		);
		const runtime = new ActionRuntime({
			confirm: () => crossRealmPromise(confirmation.promise),
		});
		const scope = runtime.createScope('application');
		runtime.register(
			scope,
			defineAction({
				category: { id: 'test', title: { id: 'action.category.test' } },
				confirmation: () => ({
					confirmLabel: { id: 'action.confirm' },
					description: { id: 'action.description' },
					risk: 'routine',
					title: { id: 'action.title' },
				}),
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'cross-realm',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: { execute: handler },
				id: 'test.cross-realm.run',
				input: noActionInput,
				title: { id: 'test.cross-realm.run.title' },
			}),
		);

		const execution = runtime.execute('test.cross-realm.run', undefined, {
			scope,
			source: 'button',
		});
		expect(execution.getSnapshot().state).toBe('awaiting-confirmation');
		expect(handler).not.toHaveBeenCalled();

		confirmation.resolve(true);
		await vi.waitFor(() => {
			expect(handler).toHaveBeenCalledOnce();
			expect(execution.getSnapshot().state).toBe('running');
		});

		handlerCompletion.resolve({ kind: 'completed', value: 'done' });
		await expect(execution.completion).resolves.toEqual({
			kind: 'completed',
			value: 'done',
		});
		expect(execution.getSnapshot()).toMatchObject({
			result: { kind: 'completed', value: 'done' },
			state: 'completed',
		});
	});
});
