import { ActionRuntime, defineAction, noActionInput } from '@orchbee/actions';
import { describe, expect, it } from 'vitest';

import { executePreparedDescendantAction } from '../executePreparedDescendantAction';

describe('executePreparedDescendantAction', () => {
	it('preserves the lower terminal result while removing its unrelated value', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('parent');
		const childScope = runtime.createScope('child', parentScope);
		runtime.register(
			childScope,
			defineAction({
				category: { id: 'test', title: { defaultMessage: 'Test' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'lower',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (context) => {
						context.markCommitted();
						return { kind: 'completed', value: 'lower-value' };
					},
				},
				id: 'test.lower',
				input: noActionInput,
				title: { defaultMessage: 'Lower' },
			}),
		);
		const prepared = runtime
			.discover(
				{ scope: parentScope, source: 'keyboard' },
				{ includeDescendants: true, inputByActionId: { 'test.lower': undefined } },
			)
			.find((result) => result.kind === 'ready' && result.descriptor.id === 'test.lower');
		if (prepared?.kind !== 'ready') throw new Error('Expected a prepared lower Action.');
		runtime.register(
			parentScope,
			defineAction({
				category: { id: 'test', title: { defaultMessage: 'Test' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'parent',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: async (context) =>
						await executePreparedDescendantAction(context, runtime, prepared.invocation),
				},
				id: 'test.parent',
				input: noActionInput,
				title: { defaultMessage: 'Parent' },
			}),
		);

		await expect(
			runtime.execute('test.parent', undefined, {
				scope: parentScope,
				source: 'keyboard',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });
	});
});
