// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type {
	WorkbenchWorkspaceFilesSource,
	WorkspaceFilesWatchReport,
	WorkspaceFilesWatchScope,
} from '../../../workbench';
import { type WorkspaceFilesChange, useWorkspaceFilesWatch } from '../useWorkspaceFilesWatch';

/** A source with one watch at a time, whose reports the test sends. */
function watches(): {
	readonly source: WorkbenchWorkspaceFilesSource;
	readonly scopes: WorkspaceFilesWatchScope[];
	readonly open: () => boolean;
	report(report: WorkspaceFilesWatchReport): void;
} {
	const scopes: WorkspaceFilesWatchScope[] = [];
	let onReport: ((report: WorkspaceFilesWatchReport) => void) | null = null;
	return {
		open: () => onReport !== null,
		report: (report) => act(() => onReport?.(report)),
		scopes,
		source: {
			read: () => Promise.resolve(null),
			save: () => Promise.resolve(null),
			watch(_workspaceId, scope, receive) {
				scopes.push(scope);
				onReport = receive;
				return {
					close: () => {
						onReport = null;
					},
					update: (next) => scopes.push(next),
				};
			},
		},
	};
}

const SCOPE: WorkspaceFilesWatchScope = { directories: [''], files: [] };
const FILES = { files: true, repository: false, state: 'watching' } as const;
const REPOSITORY = { files: false, repository: true, state: 'watching' } as const;

let hidden = false;
beforeEach(() => {
	vi.useFakeTimers();
	hidden = false;
	vi.spyOn(document, 'hidden', 'get').mockImplementation(() => hidden);
});
afterEach(() => {
	cleanup();
	vi.useRealTimers();
	vi.restoreAllMocks();
});

describe('useWorkspaceFilesWatch', () => {
	it('reads a burst of changes once, a second after the last', () => {
		const { report, source } = watches();
		const onChange = vi.fn<(change: WorkspaceFilesChange) => void>();
		const { result } = renderHook(() => useWorkspaceFilesWatch(source, 'w', SCOPE, onChange));
		report({ files: false, repository: false, state: 'watching' });
		expect(result.current.state).toBe('watching');
		report(FILES);
		act(() => vi.advanceTimersByTime(900));
		report(REPOSITORY);
		act(() => vi.advanceTimersByTime(900));
		expect(onChange).not.toHaveBeenCalled();
		act(() => vi.advanceTimersByTime(100));
		expect(onChange.mock.calls).toEqual([[{ files: true, repository: true }]]);
	});

	it('still reads a burst that never goes quiet every five seconds', () => {
		const { report, source } = watches();
		const onChange = vi.fn<(change: WorkspaceFilesChange) => void>();
		renderHook(() => useWorkspaceFilesWatch(source, 'w', SCOPE, onChange));
		for (let elapsed = 0; elapsed < 5000; elapsed += 500) {
			report(FILES);
			act(() => vi.advanceTimersByTime(500));
		}
		expect(onChange).toHaveBeenCalledTimes(1);
	});

	it('holds changes while the window is hidden and reads them when it shows', () => {
		const { report, source } = watches();
		const onChange = vi.fn<(change: WorkspaceFilesChange) => void>();
		const { result } = renderHook(() => useWorkspaceFilesWatch(source, 'w', SCOPE, onChange));
		hidden = true;
		report(FILES);
		act(() => result.current.notify({ files: false, repository: true }));
		act(() => vi.advanceTimersByTime(10_000));
		expect(onChange).not.toHaveBeenCalled();
		hidden = false;
		act(() => document.dispatchEvent(new Event('visibilitychange')));
		expect(onChange.mock.calls).toEqual([[{ files: true, repository: true }]]);
	});

	it('keeps one watch while the scope changes and closes it when nothing shows', () => {
		const { open, scopes, source } = watches();
		const { rerender } = renderHook(
			({ scope }: { scope: WorkspaceFilesWatchScope | null }) =>
				useWorkspaceFilesWatch(source, 'w', scope, () => undefined),
			{ initialProps: { scope: SCOPE as WorkspaceFilesWatchScope | null } },
		);
		const wider = { directories: ['', 'src'], files: ['a.txt'] };
		rerender({ scope: wider });
		expect(scopes.at(-1)).toEqual(wider);
		expect(open()).toBe(true);
		rerender({ scope: null });
		expect(open()).toBe(false);
	});
});
