import { RowPromptClass } from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../projected-search-worker';
import type { ProjectedSearchWorkerResponse } from '../projected-search-protocol';

afterEach(() => vi.unstubAllGlobals());

describe('projected search Worker', () => {
	it('rejects pages that do not belong to the active immutable scan', () => {
		const postMessage = vi.fn<(message: ProjectedSearchWorkerResponse) => void>();
		vi.stubGlobal('postMessage', postMessage);

		globalThis.onmessage?.call(
			globalThis as unknown as Window,
			new MessageEvent('message', {
				data: {
					final: true,
					generation: '1',
					kind: 'page',
					revision: '1',
					rows: [],
					searchId: 7,
				},
			}),
		);

		expect(postMessage).toHaveBeenCalledWith({
			code: 'invalid-sequence',
			kind: 'error',
			message: 'Search page does not match the active scan.',
			searchId: 7,
		});
	});

	it('returns matches only after the final contiguous page', () => {
		const postMessage = vi.fn<(message: ProjectedSearchWorkerResponse) => void>();
		vi.stubGlobal('postMessage', postMessage);
		globalThis.onmessage?.call(
			globalThis as unknown as Window,
			new MessageEvent('message', {
				data: {
					generation: '2',
					kind: 'start',
					navigation: { preserveStart: true, reverse: true },
					options: {},
					query: 'bee',
					revision: '9',
					searchId: 8,
				},
			}),
		);
		globalThis.onmessage?.call(
			globalThis as unknown as Window,
			new MessageEvent('message', {
				data: {
					final: true,
					generation: '2',
					kind: 'page',
					revision: '9',
					rows: [
						{
							cells: null,
							identityAscii: true,
							index: 0n,
							mappings: [],
							promptClass: RowPromptClass.none,
							text: 'orchbee',
							wrap: false,
							wrapContinuation: false,
						},
					],
					searchId: 8,
				},
			}),
		);

		expect(postMessage).toHaveBeenCalledWith(
			expect.objectContaining({
				activeMatch: expect.objectContaining({ start: { column: 4, row: 0 } }),
				generation: '2',
				kind: 'result',
				revision: '9',
				searchId: 8,
			}),
		);
	});

	it('credits bounded pages without publishing partial results before a source-limit failure', () => {
		const postMessage = vi.fn<(message: ProjectedSearchWorkerResponse) => void>();
		vi.stubGlobal('postMessage', postMessage);
		globalThis.onmessage?.call(
			globalThis as unknown as Window,
			new MessageEvent('message', {
				data: {
					generation: '3',
					kind: 'start',
					navigation: { preserveStart: true, reverse: true },
					options: {},
					query: 'missing',
					revision: '10',
					searchId: 9,
				},
			}),
		);
		const oneMebibyte = 'x'.repeat(1024 * 1024);
		for (let index = 0; index < 16; index++) {
			postWorkerPage(index, oneMebibyte, false, 9);
		}

		expect(postMessage.mock.calls.filter(([message]) => message.kind === 'progress')).toHaveLength(
			16,
		);
		expect(postMessage.mock.calls.some(([message]) => message.kind === 'result')).toBe(false);

		postWorkerPage(16, 'x', true, 9);

		expect(postMessage).toHaveBeenLastCalledWith({
			code: 'source-too-large',
			kind: 'error',
			message: 'Search source exceeds its bounded limit.',
			searchId: 9,
		});
		expect(postMessage.mock.calls.some(([message]) => message.kind === 'result')).toBe(false);
	});
});

function postWorkerPage(index: number, text: string, final: boolean, searchId: number): void {
	globalThis.onmessage?.call(
		globalThis as unknown as Window,
		new MessageEvent('message', {
			data: {
				final,
				generation: '3',
				kind: 'page',
				revision: '10',
				rows: [
					{
						cells: null,
						identityAscii: true,
						index: BigInt(index),
						mappings: [],
						promptClass: RowPromptClass.none,
						text,
						wrap: false,
						wrapContinuation: false,
					},
				],
				searchId,
			},
		}),
	);
}
