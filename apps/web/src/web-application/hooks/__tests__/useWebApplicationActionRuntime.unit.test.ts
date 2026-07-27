// @vitest-environment jsdom

import { type ActionRuntime } from '@orchbee/actions';
import { act, createElement, StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWebApplicationActionRuntime } from '../useWebApplicationActionRuntime';

function RuntimeHarness({ capture }: { readonly capture: (runtime: ActionRuntime) => void }) {
	const runtime = useWebApplicationActionRuntime();
	capture(runtime);
	return null;
}

describe('useWebApplicationActionRuntime', () => {
	it('creates one runtime for each mounted application composition', () => {
		const runtimes: ActionRuntime[] = [];
		const capture = (runtime: ActionRuntime) => runtimes.push(runtime);

		renderToStaticMarkup(createElement(RuntimeHarness, { capture }));
		renderToStaticMarkup(createElement(RuntimeHarness, { capture }));

		expect(runtimes).toHaveLength(2);
		expect(runtimes[0]).not.toBe(runtimes[1]);
		expect(runtimes[0]?.createScope('first')).toMatchObject({ id: 'first' });
		expect(runtimes[1]?.createScope('second')).toMatchObject({ id: 'second' });
	});

	it('survives StrictMode effect replay and disposes after a real unmount', async () => {
		let runtime: ActionRuntime | undefined;
		const container = document.createElement('div');
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					StrictMode,
					null,
					createElement(RuntimeHarness, {
						capture: (candidate) => {
							runtime = candidate;
						},
					}),
				),
			);
		});
		if (runtime === undefined) {
			throw new Error('Expected the mounted runtime to be captured');
		}

		expect(runtime.createScope('strict-mode-survivor')).toMatchObject({
			id: 'strict-mode-survivor',
		});

		await act(async () => {
			root.unmount();
		});
		await Promise.resolve();

		expect(() => runtime?.createScope('after-unmount')).toThrow('Action runtime is disposed');
	});
});
