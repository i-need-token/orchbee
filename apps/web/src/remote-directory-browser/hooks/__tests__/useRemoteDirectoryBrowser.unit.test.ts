// @vitest-environment jsdom

import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { useRemoteDirectoryBrowser } from '../useRemoteDirectoryBrowser';

describe('useRemoteDirectoryBrowser', () => {
	it('owns path-editor disclosure, reset, and bounded navigation submission', () => {
		const onNavigate = vi.fn<(path: string) => void>();
		const onPathDraftChange = vi.fn<(path: string) => void>();
		const { result, rerender } = renderHook(
			({ busy, canNavigate, pathDraft }) =>
				useRemoteDirectoryBrowser({
					busy,
					canNavigate,
					canonicalPath: '/home/builder',
					directories: [],
					navigationFailure: null,
					onNavigate,
					onPathDraftChange,
					pathDraft,
				}),
			{
				initialProps: { busy: false, canNavigate: true, pathDraft: '/srv/orchbee' },
			},
		);

		act(() => result.current.openPathEditor());
		expect(result.current.showPathEditor).toBe(true);
		expect(onPathDraftChange).toHaveBeenLastCalledWith('/home/builder/');

		act(() => result.current.resetPathEditor());
		expect(result.current.showPathEditor).toBe(false);
		expect(onPathDraftChange).toHaveBeenLastCalledWith('/home/builder');

		rerender({ busy: true, canNavigate: true, pathDraft: '/srv/orchbee' });
		act(() => result.current.submitPath());
		expect(onNavigate).not.toHaveBeenCalled();

		rerender({ busy: false, canNavigate: true, pathDraft: '/srv/orchbee' });
		act(() => result.current.submitPath());
		expect(onNavigate).toHaveBeenCalledWith('/srv/orchbee');
	});

	it('keeps recovery editing visible while a navigation failure exists', () => {
		const onPathDraftChange = vi.fn<(path: string) => void>();
		const { result } = renderHook(() =>
			useRemoteDirectoryBrowser({
				busy: false,
				canNavigate: true,
				canonicalPath: '/home/builder',
				directories: [],
				navigationFailure: 'not-found',
				onNavigate: () => undefined,
				onPathDraftChange,
				pathDraft: '/missing',
			}),
		);

		expect(result.current.showPathEditor).toBe(true);
		act(() => result.current.resetPathEditor());
		expect(result.current.showPathEditor).toBe(false);
		expect(onPathDraftChange).toHaveBeenLastCalledWith('/home/builder');
	});
});
