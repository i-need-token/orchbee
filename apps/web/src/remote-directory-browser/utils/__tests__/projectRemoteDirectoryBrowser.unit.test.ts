import { describe, expect, it } from 'vitest';

import { projectRemoteDirectoryBrowser } from '../projectRemoteDirectoryBrowser';

const directories = [
	{ name: '.config', path: '/home/builder/.config' },
	{ name: 'github', path: '/home/builder/github' },
	{ name: 'go', path: '/home/builder/go' },
	{ name: 'google-cloud-sdk', path: '/home/builder/google-cloud-sdk' },
] as const;

describe('projectRemoteDirectoryBrowser', () => {
	it('hides dot folders by default and reports the available disclosure', () => {
		const projection = projectRemoteDirectoryBrowser({
			canNavigate: true,
			canonicalPath: '/home/builder',
			directories,
			pathDraft: '/home/builder',
			pathEditorOpen: false,
			showHiddenDirectories: false,
		});

		expect(projection.hasHiddenDirectories).toBe(true);
		expect(projection.visibleDirectories.map(({ name }) => name)).toEqual([
			'github',
			'go',
			'google-cloud-sdk',
		]);
	});

	it('filters direct-child candidates and blocks an ambiguous partial path', () => {
		const projection = projectRemoteDirectoryBrowser({
			canNavigate: true,
			canonicalPath: '/home/builder',
			directories,
			pathDraft: '/home/builder/g',
			pathEditorOpen: true,
			showHiddenDirectories: false,
		});

		expect(projection.canSubmitPath).toBe(false);
		expect(projection.pathFilterState).toBe('partial');
		expect(projection.visibleDirectories.map(({ name }) => name)).toEqual([
			'github',
			'go',
			'google-cloud-sdk',
		]);
	});

	it('accepts an exact candidate and preserves direct navigation for an unlisted complete path', () => {
		const exact = projectRemoteDirectoryBrowser({
			canNavigate: true,
			canonicalPath: '/home/builder',
			directories,
			pathDraft: '/home/builder/go',
			pathEditorOpen: true,
			showHiddenDirectories: false,
		});
		const unlisted = projectRemoteDirectoryBrowser({
			canNavigate: true,
			canonicalPath: '/home/builder',
			directories,
			pathDraft: '/srv/orchbee',
			pathEditorOpen: true,
			showHiddenDirectories: false,
		});

		expect(exact).toMatchObject({ canSubmitPath: true, pathFilterState: 'exact' });
		expect(unlisted).toMatchObject({ canSubmitPath: true, pathFilterState: 'inactive' });
	});

	it('reveals matching dot folders from explicit path input without changing the toggle', () => {
		const projection = projectRemoteDirectoryBrowser({
			canNavigate: true,
			canonicalPath: '/home/builder',
			directories,
			pathDraft: '/home/builder/.c',
			pathEditorOpen: true,
			showHiddenDirectories: false,
		});

		expect(projection.visibleDirectories).toEqual([directories[0]]);
		expect(projection.pathFilterState).toBe('partial');
	});
});
