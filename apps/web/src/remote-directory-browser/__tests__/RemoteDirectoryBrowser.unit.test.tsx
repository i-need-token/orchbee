// @vitest-environment jsdom

import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useState, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { RemoteDirectoryBrowser } from '../RemoteDirectoryBrowser';
import type {
	RemoteDirectoryBrowserLocation,
	RemoteDirectoryBrowserProps,
} from '../types/remote-directory-browser';

afterEach(cleanup);

describe('RemoteDirectoryBrowser', () => {
	it.each([
		{ locale: 'en-US', useFolder: 'Use This Folder' },
		{ locale: 'zh-CN', useFolder: '使用此文件夹' },
	] satisfies readonly { locale: OrchbeeLocale; useFolder: string }[])(
		'ships the controlled browser in $locale',
		({ locale, useFolder }) => {
			const onNavigate = vi.fn<(path: string) => void>();
			renderBrowser({ locale, onNavigate });

			expect((document.activeElement as HTMLElement | null)?.dataset['slot']).toBe(
				'remote-directory-browser',
			);
			expect(screen.getByText('builder@build-host')).toBeTruthy();
			expect(screen.getByRole('button', { name: 'home' })).toBeTruthy();
			expect(screen.getByRole('button', { name: 'projects' })).toBeTruthy();
			expect(screen.getByRole('button', { name: useFolder })).toBeTruthy();

			fireEvent.click(screen.getByRole('button', { name: 'home' }));
			fireEvent.click(screen.getByRole('button', { name: 'projects' }));
			expect(onNavigate.mock.calls).toEqual([['/home'], ['/home/builder/projects']]);
		},
	);

	it('replaces breadcrumbs with the path editor and submits it with Enter', () => {
		const onNavigate = vi.fn<(path: string) => void>();
		renderBrowser({ onNavigate });

		expect(screen.getByRole('navigation', { name: 'Current remote location' })).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Edit folder path' }));
		expect(screen.queryByRole('navigation', { name: 'Current remote location' })).toBeNull();
		expect(screen.queryByText('builder@build-host')).toBeNull();
		const pathInput = screen.getByLabelText('Folder path');
		expect((pathInput as HTMLInputElement).value).toBe('/home/builder/');
		const go = screen.getByRole('button', { name: 'Go' });
		expect(screen.queryByRole('button', { name: 'Use This Folder' })).toBeNull();
		fireEvent.change(pathInput, { target: { value: '/srv/orchbee' } });
		fireEvent.blur(pathInput, { relatedTarget: go });
		expect(screen.getByLabelText('Folder path')).toBeTruthy();
		fireEvent.click(go);

		expect(onNavigate).toHaveBeenCalledWith('/srv/orchbee');
	});

	it('advertises and executes the outer Escape and Enter actions', () => {
		const onCancel = vi.fn<() => void>();
		const onSelect = vi.fn<() => void>();
		renderBrowser({ onCancel, onSelect });

		const cancel = screen.getByRole('button', { name: 'Cancel' });
		const select = screen.getByRole('button', { name: 'Use This Folder' });
		expect(cancel.textContent).toContain('Esc');
		expect(select.textContent).toContain('Enter');
		const browser = document.querySelector<HTMLElement>('[data-slot="remote-directory-browser"]');
		if (browser === null) throw new Error('Expected the remote Directory Browser.');
		fireEvent.keyDown(browser, { key: 'Enter' });
		expect(onSelect).toHaveBeenCalledOnce();
		expect(onCancel).not.toHaveBeenCalled();
	});

	it('restores the canonical breadcrumb representation when path editing receives Escape', () => {
		const onNavigate = vi.fn<(path: string) => void>();
		renderBrowser({ onNavigate });

		fireEvent.click(screen.getByRole('button', { name: 'Edit folder path' }));
		const pathInput = screen.getByLabelText('Folder path');
		fireEvent.change(pathInput, { target: { value: '/srv/orchbee' } });
		fireEvent.keyDown(pathInput, { key: 'Escape' });

		expect(screen.queryByLabelText('Folder path')).toBeNull();
		expect(screen.getByRole('navigation', { name: 'Current remote location' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Use This Folder' })).toBeTruthy();
		expect(onNavigate).not.toHaveBeenCalled();

		fireEvent.click(screen.getByRole('button', { name: 'Edit folder path' }));
		const reopenedPathInput = screen.getByLabelText('Folder path');
		expect((reopenedPathInput as HTMLInputElement).value).toBe('/home/builder/');
		fireEvent.change(reopenedPathInput, { target: { value: '/home/builder/pro' } });
		fireEvent.blur(reopenedPathInput);
		expect(screen.queryByLabelText('Folder path')).toBeNull();
		expect(screen.getByRole('navigation', { name: 'Current remote location' })).toBeTruthy();
	});

	it('filters current child folders from a partial path and connects an exact row selection', () => {
		const onNavigate = vi.fn<(path: string) => void>();
		renderBrowser({
			location: {
				...location(),
				directories: [
					{ name: '.config', path: '/home/builder/.config' },
					{ name: 'github', path: '/home/builder/github' },
					{ name: 'go', path: '/home/builder/go' },
					{ name: 'google-cloud-sdk', path: '/home/builder/google-cloud-sdk' },
				],
			},
			onNavigate,
		});

		expect(screen.queryByRole('button', { name: '.config' })).toBeNull();
		expect(screen.getByText('Show hidden folders')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Edit folder path' }));
		const pathInput = screen.getByLabelText('Folder path');
		fireEvent.change(pathInput, {
			target: { value: '/home/builder/g' },
		});

		expect(screen.getByText(/finish entering its complete path/u)).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Go' }).hasAttribute('disabled')).toBe(true);
		const github = screen.getByRole('button', { name: 'github' });
		expect(github).toBeTruthy();
		expect(screen.getByRole('button', { name: 'google-cloud-sdk' })).toBeTruthy();
		const go = screen.getByRole('button', { name: 'go' });
		fireEvent.blur(pathInput, { relatedTarget: go });
		expect(screen.getByLabelText('Folder path')).toBeTruthy();
		fireEvent.click(go);
		expect(onNavigate).toHaveBeenCalledWith('/home/builder/go');
	});

	it('reveals hidden folders only when requested', () => {
		renderBrowser({
			location: {
				...location(),
				directories: [{ name: '.config', path: '/home/builder/.config' }],
			},
		});

		expect(screen.queryByRole('button', { name: '.config' })).toBeNull();
		expect(screen.getByText(/no visible subfolders/u)).toBeTruthy();
		fireEvent.click(screen.getByRole('checkbox', { name: 'Show hidden folders' }));
		expect(screen.getByRole('button', { name: '.config' })).toBeTruthy();
	});

	it('keeps stale folders visible and disables decisions while a navigation is loading', () => {
		renderBrowser({ busy: true });

		expect(screen.getByRole('status').textContent).toContain('Loading folder');
		expect(document.querySelector('[data-slot="remote-directory-loading-indicator"]')).toBeTruthy();
		const folder = screen.getByRole('button', { name: 'projects' });
		expect(folder.hasAttribute('disabled')).toBe(true);
		expect(screen.getByRole('button', { name: 'Use This Folder' }).hasAttribute('disabled')).toBe(
			true,
		);
	});

	it('shows bounded failure, empty, and truncation states without discarding the location', () => {
		renderBrowser({
			location: {
				...location(),
				directories: [],
				navigationFailure: 'permission-denied',
				truncated: true,
			},
			pathDraft: '/root',
		});

		expect(screen.getByRole('alert').textContent).toContain('permission');
		expect(screen.getByText('This folder has no accessible subfolders.')).toBeTruthy();
		expect(screen.getByText(/Some folders are not shown/u)).toBeTruthy();
		expect((screen.getByLabelText('Folder path') as HTMLInputElement).value).toBe('/root');
		expect(screen.queryByText('builder@build-host')).toBeNull();
	});
});

interface RenderBrowserOptions {
	readonly busy?: boolean;
	readonly locale?: OrchbeeLocale;
	readonly location?: RemoteDirectoryBrowserLocation;
	readonly onCancel?: () => void;
	readonly onNavigate?: (path: string) => void;
	readonly onSelect?: () => void;
	readonly pathDraft?: string;
}

function renderBrowser(options: RenderBrowserOptions = {}): void {
	const locale = options.locale ?? 'en-US';
	render(
		<OrchbeeI18nProvider locale={locale}>
			<BrowserHarness
				busy={options.busy ?? false}
				location={options.location ?? location()}
				onCancel={options.onCancel ?? (() => undefined)}
				onNavigate={options.onNavigate ?? (() => undefined)}
				onSelect={options.onSelect ?? (() => undefined)}
				pathDraft={options.pathDraft ?? '/home/builder'}
			/>
		</OrchbeeI18nProvider>,
	);
}

function BrowserHarness({
	busy,
	location: browserLocation,
	onCancel,
	onNavigate,
	onSelect,
	pathDraft: initialPathDraft,
}: Pick<
	RemoteDirectoryBrowserProps,
	'busy' | 'location' | 'onCancel' | 'onNavigate' | 'onSelect' | 'pathDraft'
>): ReactNode {
	const [pathDraft, setPathDraft] = useState(initialPathDraft);
	return (
		<RemoteDirectoryBrowser
			busy={busy}
			canNavigate={pathDraft.startsWith('/')}
			location={browserLocation}
			onCancel={onCancel}
			onNavigate={onNavigate}
			onPathDraftChange={setPathDraft}
			onSelect={onSelect}
			pathDraft={pathDraft}
			sourceLabel='builder@build-host'
		/>
	);
}

function location(): RemoteDirectoryBrowserLocation {
	return {
		canonicalPath: '/home/builder',
		crumbs: [
			{ label: '/', path: '/' },
			{ label: 'home', path: '/home' },
			{ label: 'builder', path: '/home/builder' },
		],
		directories: [{ name: 'projects', path: '/home/builder/projects' }],
		navigationFailure: null,
		truncated: false,
	};
}
