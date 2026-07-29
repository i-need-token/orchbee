// @vitest-environment jsdom

import { createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { flushSync } from 'react-dom';
import { describe, expect, it } from 'vitest';

import { useMemoizedFn } from '../useMemoizedFn';

type NumberFunction = (increment: number) => number;

describe('useMemoizedFn', () => {
	it('keeps one function identity while invoking the latest implementation', () => {
		const container = document.createElement('div');
		const root = createRoot(container);
		let currentFn: NumberFunction | undefined;
		const Harness = ({ value }: { readonly value: number }) => {
			currentFn = useMemoizedFn((increment: number) => value + increment);
			return null;
		};

		try {
			flushSync(() => {
				root.render(createElement(Harness, { value: 1 }));
			});
			const memoizedFn = currentFn;
			if (memoizedFn === undefined) {
				throw new Error('Expected the memoized function to be captured');
			}
			expect(memoizedFn(2)).toBe(3);

			flushSync(() => {
				root.render(createElement(Harness, { value: 5 }));
			});

			expect(currentFn).toBe(memoizedFn);
			expect(memoizedFn(2)).toBe(7);
		} finally {
			flushSync(() => {
				root.unmount();
			});
		}
	});

	it('preserves the receiver supplied by the caller', () => {
		const container = document.createElement('div');
		const root = createRoot(container);
		let currentFn: ((this: { readonly base: number }, increment: number) => number) | undefined;
		const Harness = () => {
			currentFn = useMemoizedFn(function (this: { readonly base: number }, increment: number) {
				return this.base + increment;
			});
			return null;
		};

		try {
			flushSync(() => {
				root.render(createElement(Harness));
			});
			if (currentFn === undefined) {
				throw new Error('Expected the memoized function to be captured');
			}
			expect(currentFn.call({ base: 4 }, 3)).toBe(7);
		} finally {
			flushSync(() => {
				root.unmount();
			});
		}
	});
});
