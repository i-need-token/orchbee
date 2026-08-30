// @vitest-environment jsdom

import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	DesktopSshProjectEnrollmentController,
	type DesktopSshProjectEnrollmentControllerClient,
	type DesktopSshProjectEnrollmentControllerOptions,
} from '../model/DesktopSshProjectEnrollmentController';
import { DesktopSshProjectEnrollmentDialog } from '../DesktopSshProjectEnrollmentDialog';

afterEach(cleanup);

describe('DesktopSshProjectEnrollmentDialog', () => {
	it.each([
		{
			cancel: 'Cancel',
			connect: 'Connect',
			destination: 'SSH address',
			locale: 'en-US',
			title: 'Connect via SSH',
		},
		{
			cancel: '取消',
			connect: '连接',
			destination: 'SSH 地址',
			locale: 'zh-CN',
			title: '通过 SSH 连接',
		},
	] satisfies readonly {
		cancel: string;
		connect: string;
		destination: string;
		locale: OrchbeeLocale;
		title: string;
	}[])(
		'ships the enrollment form in $locale',
		({ cancel, connect, destination, locale, title }) => {
			const controller = createController();
			controller.openEnrollment();
			render(
				<OrchbeeI18nProvider locale={locale}>
					<DesktopSshProjectEnrollmentDialog controller={controller} />
				</OrchbeeI18nProvider>,
			);

			expect(screen.getByRole('dialog')).toBeTruthy();
			expect(screen.getByRole('heading', { name: title })).toBeTruthy();
			expect(screen.getByLabelText(destination)).toBeTruthy();
			expect(screen.getByRole('button', { name: cancel }).querySelector('kbd')?.textContent).toBe(
				'Esc',
			);
			expect(screen.getByRole('button', { name: connect }).querySelector('kbd')?.textContent).toBe(
				'Enter',
			);
			controller.dispose();
		},
	);

	it.each([
		{
			error: 'Enter a valid SSH host, such as user@hostname.',
			locale: 'en-US',
		},
		{
			error: '请输入有效的 SSH 主机，例如 user@hostname。',
			locale: 'zh-CN',
		},
	] satisfies readonly { error: string; locale: OrchbeeLocale }[])(
		'explains an invalid SSH address in $locale without hiding the primary action',
		({ error, locale }) => {
			const controller = createController();
			controller.openEnrollment();
			render(
				<OrchbeeI18nProvider locale={locale}>
					<DesktopSshProjectEnrollmentDialog controller={controller} />
				</OrchbeeI18nProvider>,
			);

			const destination = screen.getByRole('textbox');
			fireEvent.click(
				screen.getByRole('button', { name: locale === 'en-US' ? 'Connect' : '连接' }),
			);

			expect(screen.getByRole('alert').textContent).toBe(error);
			expect(destination.getAttribute('aria-invalid')).toBe('true');
			expect(document.activeElement).toBe(destination);

			fireEvent.change(destination, { target: { value: 'builder@build-host' } });
			expect(screen.queryByRole('alert')).toBeNull();
			controller.dispose();
		},
	);

	it.each([
		{ locale: 'en-US', title: 'Add SSH Folder' },
		{ locale: 'zh-CN', title: '添加 SSH 文件夹' },
	] satisfies readonly { locale: OrchbeeLocale; title: string }[])(
		'ships the Folder attachment form in $locale',
		({ locale, title }) => {
			const controller = createController();
			controller.openFolderAttachment('project-local');
			render(
				<OrchbeeI18nProvider locale={locale}>
					<DesktopSshProjectEnrollmentDialog controller={controller} />
				</OrchbeeI18nProvider>,
			);
			expect(screen.getByRole('heading', { name: title })).toBeTruthy();
			controller.dispose();
		},
	);

	it('submits one address, authenticates, and chooses the remote folder before commit', async () => {
		const responses: unknown[] = [];
		const directorySelections: unknown[] = [];
		const committed = vi.fn<DesktopSshProjectEnrollmentControllerOptions['onEnrollmentCommitted']>(
			async () => true,
		);
		const controller = createController(async (_input, observer) => {
			responses.push(
				await observer.interact({
					generation: 1,
					handle: 'host-prompt',
					kind: 'confirm',
					prompt: 'The authenticity of host cannot be established.',
				}),
			);
			responses.push(
				await observer.interact({
					generation: 1,
					handle: 'key-prompt',
					kind: 'none',
					prompt: "Enter passphrase for key '/Users/me/.ssh/id_ed25519':",
				}),
			);
			directorySelections.push(
				await observer.selectDirectory({
					canonicalPath: '/srv/orchbee',
					directories: [{ name: 'packages' }],
					navigationFailure: null,
					parentPath: '/srv',
					truncated: false,
				}),
			);
			return {
				canonicalPath: '/srv/orchbee',
				kind: 'created',
				projectId: 'project-remote',
				workspaceId: 'workspace-primary',
			};
		}, committed);
		controller.openEnrollment();
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<DesktopSshProjectEnrollmentDialog controller={controller} />
			</OrchbeeI18nProvider>,
		);

		fireEvent.change(screen.getByLabelText('SSH address'), {
			target: { value: 'ssh builder@build-host' },
		});
		const addressForm = screen.getByLabelText('SSH address').closest('form');
		expect(addressForm).not.toBeNull();
		fireEvent.submit(addressForm as HTMLFormElement);

		await screen.findByRole('heading', { name: 'Verify SSH Host' });
		expect(screen.getAllByRole('heading')).toHaveLength(1);
		expect(screen.getByText('The authenticity of host cannot be established.')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Cancel' }).querySelector('kbd')?.textContent).toBe(
			'Esc',
		);
		const trustHost = screen.getByRole('button', { name: 'Trust and Continue' });
		expect(trustHost.querySelector('kbd')).toBeNull();
		fireEvent.click(trustHost);

		await screen.findByRole('heading', { name: 'SSH Authentication Required' });
		expect(screen.getAllByRole('heading')).toHaveLength(1);
		const response = screen.getByLabelText('Password or passphrase');
		expect(response.getAttribute('type')).toBe('password');
		fireEvent.change(response, { target: { value: 'one-use secret' } });
		expect(JSON.stringify(controller.getDialogSnapshot())).not.toContain('one-use secret');
		const authenticate = screen.getByRole('button', { name: 'Authenticate' });
		expect(authenticate.querySelector('kbd')?.textContent).toBe('Enter');
		fireEvent.click(authenticate);

		await screen.findByRole('heading', { name: 'Choose a Project Folder' });
		fireEvent.click(screen.getByRole('button', { name: 'Edit folder path' }));
		expect((screen.getByLabelText('Folder path') as HTMLInputElement).value).toBe('/srv/orchbee');
		expect(screen.getByRole('button', { name: /packages/u })).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Use This Folder' }));

		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(responses).toEqual([
			{ kind: 'submitted', response: 'yes' },
			{ kind: 'submitted', response: 'one-use secret' },
		]);
		expect(directorySelections).toEqual([{ kind: 'selected' }]);
		expect(committed).toHaveBeenCalledOnce();
		controller.dispose();
	});
});

function createController(
	enroll: DesktopSshProjectEnrollmentControllerClient['enroll'] = async () => ({
		kind: 'cancelled',
	}),
	onEnrollmentCommitted: DesktopSshProjectEnrollmentControllerOptions['onEnrollmentCommitted'] = async () =>
		true,
): DesktopSshProjectEnrollmentController {
	const client: DesktopSshProjectEnrollmentControllerClient = {
		addEnvironment: async () => ({ kind: 'cancelled' }),
		attachFolder: async () => ({ kind: 'cancelled' }),
		connect: async (projectId) => ({ kind: 'connected', projectId }),
		connectEnvironment: async () => ({ kind: 'cancelled' }),
		disconnect: async (projectId) => ({ kind: 'disconnected', projectId }),
		disconnectEnvironment: async () => ({ kind: 'cancelled' }),
		dispose: () => undefined,
		enroll,
		getEnvironmentCatalog: () => ({
			discoveryIssue: null,
			managed: [],
			registryIssue: null,
			suggestions: [],
		}),
		getStatuses: () => [],
		refreshEnvironmentCatalog: () => undefined,
		subscribeEnvironmentCatalog: () => () => undefined,
		subscribeStatuses: () => () => undefined,
	};
	return new DesktopSshProjectEnrollmentController({
		client,
		onEnrollmentCommitted,
		onFolderAttachmentCommitted: async () => true,
	});
}
