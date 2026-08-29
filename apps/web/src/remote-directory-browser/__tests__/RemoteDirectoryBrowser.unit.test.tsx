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

			expect(screen.getByText('builder@build-host')).toBeTruthy();
			expect(screen.getByRole('button', { name: 'home' })).toBeTruthy();
			expect(screen.getByRole('button', { name: 'projects' })).toBeTruthy();
			expect(screen.getByRole('button', { name: useFolder })).toBeTruthy();

			fireEvent.click(screen.getByRole('button', { name: 'home' }));
			fireEvent.click(screen.getByRole('button', { name: 'projects' }));
			expect(onNavigate.mock.calls).toEqual([['/home'], ['/home/builder/projects']]);
		},
	);

	it('opens an editable path and submits it with Enter', () => {
		const onNavigate = vi.fn<(path: string) => void>();
		renderBrowser({ onNavigate });

		fireEvent.click(screen.getByRole('button', { name: 'Edit folder path' }));
		const pathInput = screen.getByLabelText('Folder path');
		expect((pathInput as HTMLInputElement).value).toBe('/home/builder');
		fireEvent.change(pathInput, { target: { value: '/srv/orchbee' } });
		fireEvent.submit(pathInput.closest('form') as HTMLFormElement);

		expect(onNavigate).toHaveBeenCalledWith('/srv/orchbee');
	});

	it('keeps stale folders visible and disables decisions while a navigation is loading', () => {
		renderBrowser({ busy: true });

		expect(screen.getByRole('status').textContent).toContain('Loading folder');
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
		expect(screen.getByText('builder')).toBeTruthy();
	});
});

interface RenderBrowserOptions {
	readonly busy?: boolean;
	readonly locale?: OrchbeeLocale;
	readonly location?: RemoteDirectoryBrowserLocation;
	readonly onNavigate?: (path: string) => void;
	readonly pathDraft?: string;
}

function renderBrowser(options: RenderBrowserOptions = {}): void {
	const locale = options.locale ?? 'en-US';
	render(
		<OrchbeeI18nProvider locale={locale}>
			<BrowserHarness
				busy={options.busy ?? false}
				location={options.location ?? location()}
				onNavigate={options.onNavigate ?? (() => undefined)}
				pathDraft={options.pathDraft ?? '/home/builder'}
			/>
		</OrchbeeI18nProvider>,
	);
}

function BrowserHarness({
	busy,
	location: browserLocation,
	onNavigate,
	pathDraft: initialPathDraft,
}: Pick<RemoteDirectoryBrowserProps, 'busy' | 'location' | 'onNavigate' | 'pathDraft'>): ReactNode {
	const [pathDraft, setPathDraft] = useState(initialPathDraft);
	return (
		<RemoteDirectoryBrowser
			busy={busy}
			canNavigate={pathDraft.startsWith('/')}
			location={browserLocation}
			onCancel={() => undefined}
			onNavigate={onNavigate}
			onPathDraftChange={setPathDraft}
			onSelect={() => undefined}
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
