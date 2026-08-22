import { describe, expect, it } from 'vitest';
import { EventEmitter } from '../EventEmitter';

describe('EventEmitter', () => {
	it('fires listeners synchronously and disposes subscriptions', () => {
		const emitter = new EventEmitter<number>();
		const values: number[] = [];
		const first = emitter.event((value) => values.push(value));
		emitter.event((value) => values.push(value * 10));
		emitter.fire(2);
		first.dispose();
		emitter.fire(3);
		expect(values).toEqual([2, 20, 30]);
	});

	it('removes every listener when disposed', () => {
		const emitter = new EventEmitter<string>();
		const values: string[] = [];
		emitter.event((value) => values.push(value));
		emitter.dispose();
		emitter.fire('ignored');
		expect(values).toEqual([]);
	});

	it('finishes the listener snapshot when a callback unsubscribes a sibling', () => {
		const emitter = new EventEmitter<string>();
		const values: string[] = [];
		let secondSubscription: { dispose(): void } | undefined;
		emitter.event((value) => {
			values.push(`first:${value}`);
			secondSubscription?.dispose();
		});
		secondSubscription = emitter.event((value) => values.push(`second:${value}`));

		emitter.fire('one');
		emitter.fire('two');

		expect(values).toEqual(['first:one', 'second:one', 'first:two']);
	});
});
