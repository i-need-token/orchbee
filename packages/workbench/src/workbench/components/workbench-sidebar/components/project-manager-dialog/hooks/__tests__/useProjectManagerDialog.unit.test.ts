// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchProject } from '../../../../../../../project-workspace';
import { useProjectManagerDialog } from '../useProjectManagerDialog';

afterEach(cleanup);

describe('useProjectManagerDialog', () => {
	it('submits a trimmed unique Folder path and clears the inline draft', () => {
		const onAddDirectory = vi.fn<(path: string) => void>();
		const rendered = renderHook(() =>
			useProjectManagerDialog({ onAddDirectory, project: projectFixture() }),
		);

		act(() => rendered.result.current.showFolderInput());
		act(() => rendered.result.current.setFolderPath('  /workspace/ghostty/  '));
		expect(rendered.result.current).toMatchObject({
			addingFolder: true,
			canAddFolder: true,
			folderAlreadyAdded: false,
		});
		act(() => rendered.result.current.submitFolder());

		expect(onAddDirectory).toHaveBeenCalledOnce();
		expect(onAddDirectory).toHaveBeenCalledWith('/workspace/ghostty/');
		expect(rendered.result.current).toMatchObject({ addingFolder: false, folderPath: '' });
	});

	it('keeps an exact duplicate Folder path out of the submission callback', () => {
		const onAddDirectory = vi.fn<(path: string) => void>();
		const rendered = renderHook(() =>
			useProjectManagerDialog({ onAddDirectory, project: projectFixture() }),
		);

		act(() => rendered.result.current.showFolderInput());
		act(() => rendered.result.current.setFolderPath(' /workspace/orchbee '));
		expect(rendered.result.current).toMatchObject({
			addingFolder: true,
			canAddFolder: false,
			folderAlreadyAdded: true,
		});
		act(() => rendered.result.current.submitFolder());

		expect(onAddDirectory).not.toHaveBeenCalled();
		expect(rendered.result.current.addingFolder).toBe(true);
	});
});

function projectFixture(): WorkbenchProject {
	return {
		directories: [
			{
				available: true,
				git: null,
				id: 'directory-1',
				kind: 'folder',
				name: 'orchbee',
				path: '/workspace/orchbee',
				placement: { kind: 'local' },
				source: 'selected',
			},
		],
		id: 'project-1',
		launchConfigurations: [],
		name: 'orchbee',
		primaryDirectoryId: 'directory-1',
		workspaces: [],
	};
}
