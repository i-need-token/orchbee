// @vitest-environment jsdom

import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { WorkbenchRenderBoundary } from '../WorkbenchRenderBoundary';

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

describe('WorkbenchRenderBoundary', () => {
	it('keeps a render failure inside its subtree, reports its site, and mounts it again on retry', async () => {
		let failing = true;
		function Panel({ name }: { readonly name: string }): ReactNode {
			if (name === 'broken' && failing) throw new Error('render defect');
			return createElement('span', null, name);
		}
		let retry: (() => void) | null = null;
		const bounded = (name: string): ReactNode => (
			<WorkbenchRenderBoundary
				fallback={(again) => {
					retry = again;
					return <em>failed</em>;
				}}
				site='terminal-surface-render'
			>
				<Panel name={name} />
			</WorkbenchRenderBoundary>
		);
		const reported: unknown[] = [];
		const reportError = vi.fn<(error: unknown) => void>((error) => reported.push(error));
		vi.stubGlobal('reportError', reportError);
		// React also logs what a boundary catches; the report above is what this test reads.
		const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
		const container = document.createElement('div');
		const root = createRoot(container);
		try {
			await act(async () => {
				root.render(createElement('div', null, bounded('healthy'), bounded('broken')));
			});
			expect(container.textContent).toBe('healthyfailed');
			expect(reported).toEqual([
				expect.objectContaining({ name: 'ContainedClientFault', site: 'terminal-surface-render' }),
			]);

			failing = false;
			await act(async () => {
				retry?.();
			});
			expect(container.textContent).toBe('healthybroken');
		} finally {
			await act(async () => root.unmount());
			consoleError.mockRestore();
			vi.unstubAllGlobals();
		}
	});
});
