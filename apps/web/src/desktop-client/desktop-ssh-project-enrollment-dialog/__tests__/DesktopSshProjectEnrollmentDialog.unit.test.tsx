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
			destination: 'SSH host',
			locale: 'en-US',
			title: 'Connect via SSH',
		},
		{
			cancel: '取消',
			connect: '连接',
			destination: 'SSH 主机',
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

	it('filters inline SSH candidates and connects the selected match immediately', async () => {
		const enroll = vi
			.fn<DesktopSshProjectEnrollmentControllerClient['enroll']>()
			.mockResolvedValue({ kind: 'cancelled' });
		const controller = createController(enroll, async () => true, {
			discoveryIssue: null,
			managed: [],
			registryIssue: null,
			suggestions: [{ destination: 'github.com' }, { destination: 'gpu-lab' }],
		});
		controller.openEnrollment();
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<DesktopSshProjectEnrollmentDialog controller={controller} />
			</OrchbeeI18nProvider>,
		);

		const destination = screen.getByRole('textbox', { name: 'SSH host' });
		expect(screen.getByRole('button', { name: 'github.com' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'gpu-lab' })).toBeTruthy();
		fireEvent.change(destination, { target: { value: 'gpu' } });
		expect(screen.queryByRole('button', { name: 'github.com' })).toBeNull();
		fireEvent.click(screen.getByRole('button', { name: 'gpu-lab' }));
		await waitFor(() =>
			expect(enroll).toHaveBeenCalledWith(
				{ destination: 'gpu-lab' },
				expect.any(Object),
				expect.any(AbortSignal),
			),
		);
		controller.dispose();
	});

	it.each([
		{
			error: 'Enter a valid SSH connection command or host.',
			label: 'SSH host',
			locale: 'en-US',
		},
		{
			error: '请输入有效的 SSH 连接命令或主机。',
			label: 'SSH 主机',
			locale: 'zh-CN',
		},
	] satisfies readonly { error: string; label: string; locale: OrchbeeLocale }[])(
		'explains an invalid SSH address in $locale without hiding the primary action',
		({ error, label, locale }) => {
			const controller = createController();
			controller.openEnrollment();
			render(
				<OrchbeeI18nProvider locale={locale}>
					<DesktopSshProjectEnrollmentDialog controller={controller} />
				</OrchbeeI18nProvider>,
			);

			const destination = screen.getByRole('textbox', { name: label });
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

	it('returns Cancel and Escape to the retained SSH input while Close dismisses the Dialog', async () => {
		const interactionResults: unknown[] = [];
		let interactionCount = 0;
		const controller = createController(async (_input, observer) => {
			interactionCount += 1;
			interactionResults.push(
				await observer.interact({
					generation: interactionCount,
					handle: `authentication-${interactionCount}`,
					kind: 'none',
					prompt: 'Password:',
				}),
			);
			return { kind: 'cancelled' };
		});
		controller.openEnrollment();
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<DesktopSshProjectEnrollmentDialog controller={controller} />
			</OrchbeeI18nProvider>,
		);

		const submitDestination = (): void => {
			const destination = screen.getByLabelText('SSH host');
			fireEvent.change(destination, { target: { value: 'ssh builder@build-host -A' } });
			const form = destination.closest('form');
			expect(form).not.toBeNull();
			fireEvent.submit(form as HTMLFormElement);
		};

		submitDestination();
		await screen.findByRole('heading', { name: 'SSH Authentication Required' });
		fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		await waitFor(() => expect(interactionResults).toEqual([{ kind: 'cancelled' }]));
		expect((screen.getByLabelText('SSH host') as HTMLInputElement).value).toBe(
			'ssh builder@build-host -A',
		);

		submitDestination();
		await screen.findByRole('heading', { name: 'SSH Authentication Required' });
		fireEvent.keyDown(document, { key: 'Escape' });
		await waitFor(() =>
			expect(interactionResults).toEqual([{ kind: 'cancelled' }, { kind: 'cancelled' }]),
		);
		expect((screen.getByLabelText('SSH host') as HTMLInputElement).value).toBe(
			'ssh builder@build-host -A',
		);

		fireEvent.click(screen.getByRole('button', { name: 'Close' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		controller.dispose();
	});

	it.each([
		{ connectionLabel: 'SSH connection:', field: 'SSH host', locale: 'en-US' },
		{ connectionLabel: 'SSH 连接：', field: 'SSH 主机', locale: 'zh-CN' },
	] satisfies readonly {
		connectionLabel: string;
		field: string;
		locale: OrchbeeLocale;
	}[])(
		'keeps the exact SSH target visible while connecting in $locale',
		({ connectionLabel, field, locale }) => {
			const enroll = vi.fn<DesktopSshProjectEnrollmentControllerClient['enroll']>(
				async (_input, _observer, signal) => {
					await new Promise<void>((resolve) => {
						signal.addEventListener('abort', () => resolve(), { once: true });
					});
					return { kind: 'cancelled' };
				},
			);
			const controller = createController(enroll);
			controller.openEnrollment();
			render(
				<OrchbeeI18nProvider locale={locale}>
					<DesktopSshProjectEnrollmentDialog controller={controller} />
				</OrchbeeI18nProvider>,
			);

			const destination = screen.getByLabelText(field);
			fireEvent.change(destination, { target: { value: 'ssh builder@build-host -A' } });
			const form = destination.closest('form');
			expect(form).not.toBeNull();
			fireEvent.submit(form as HTMLFormElement);

			const connectionLabelElement = screen.getByText(connectionLabel);
			const connectionTargetElement = screen.getByText('ssh builder@build-host -A');
			expect(connectionTargetElement.closest('p')).toBe(connectionLabelElement.closest('p'));
			controller.dispose();
		},
	);

	it.each([
		{
			failure:
				'Orchbee could not establish the remote runtime through SSH connection “ssh builder@build-host -A”. Check SSH access and try again.',
			field: 'SSH host',
			locale: 'en-US',
			retry: 'Retry',
			title: 'Remote Project Could Not Be Connected',
		},
		{
			failure:
				'Orchbee 无法通过 SSH 连接“ssh builder@build-host -A”建立远端运行时。请检查 SSH 访问权限后重试。',
			field: 'SSH 主机',
			locale: 'zh-CN',
			retry: '重试',
			title: '无法连接远端项目',
		},
	] satisfies readonly {
		failure: string;
		field: string;
		locale: OrchbeeLocale;
		retry: string;
		title: string;
	}[])(
		'identifies the failed SSH command and invokes Retry with Enter in $locale',
		async ({ failure, field, locale, retry: retryLabel, title }) => {
			const enroll = vi
				.fn<DesktopSshProjectEnrollmentControllerClient['enroll']>()
				.mockResolvedValue({ kind: 'failed', reason: 'remote-runtime-unavailable' });
			const controller = createController(enroll);
			controller.openEnrollment();
			render(
				<OrchbeeI18nProvider locale={locale}>
					<DesktopSshProjectEnrollmentDialog controller={controller} />
				</OrchbeeI18nProvider>,
			);

			const destination = screen.getByLabelText(field);
			fireEvent.change(destination, { target: { value: 'ssh builder@build-host -A' } });
			const form = destination.closest('form');
			expect(form).not.toBeNull();
			fireEvent.submit(form as HTMLFormElement);

			await screen.findByRole('heading', { name: title });
			expect(screen.getByRole('alert').textContent).toBe(failure);
			const retry = screen.getByRole('button', { name: retryLabel });
			expect(retry.querySelector('kbd')?.textContent).toBe('Enter');
			expect(document.activeElement).toBe(retry);

			fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Enter' });
			await waitFor(() => expect(enroll).toHaveBeenCalledTimes(2));
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

		fireEvent.change(screen.getByLabelText('SSH host'), {
			target: { value: 'ssh builder@build-host' },
		});
		const addressForm = screen.getByLabelText('SSH host').closest('form');
		expect(addressForm).not.toBeNull();
		fireEvent.submit(addressForm as HTMLFormElement);

		await screen.findByRole('heading', { name: 'Verify SSH Host' });
		expect(screen.getAllByRole('heading')).toHaveLength(1);
		const hostPrompt = screen.getByText('The authenticity of host cannot be established.');
		expect(hostPrompt.closest('pre')).not.toBeNull();
		expect(screen.getByRole('button', { name: 'Cancel' }).querySelector('kbd')?.textContent).toBe(
			'Esc',
		);
		const trustHost = screen.getByRole('button', { name: 'Trust and Continue' });
		expect(trustHost.querySelector('kbd')).toBeNull();
		fireEvent.click(trustHost);

		await screen.findByRole('heading', { name: 'SSH Authentication Required' });
		expect(screen.getAllByRole('heading')).toHaveLength(1);
		const authenticationPrompt = screen.getByText(
			"Enter passphrase for key '/Users/me/.ssh/id_ed25519':",
		);
		expect(authenticationPrompt.closest('[data-slot="dialog-description"]')).not.toBeNull();
		expect(authenticationPrompt.closest('pre')).toBeNull();
		const response = screen.getByLabelText('Password or passphrase');
		expect(response.getAttribute('type')).toBe('password');
		fireEvent.change(response, { target: { value: 'one-use secret' } });
		expect(JSON.stringify(controller.getDialogSnapshot())).not.toContain('one-use secret');
		const authenticate = screen.getByRole('button', { name: 'Authenticate' });
		expect(authenticate.querySelector('kbd')?.textContent).toBe('Enter');
		fireEvent.click(authenticate);

		await screen.findByRole('heading', { name: 'Choose a Project Folder' });
		fireEvent.click(screen.getByRole('button', { name: 'Edit folder path' }));
		const folderPath = screen.getByLabelText('Folder path');
		expect((folderPath as HTMLInputElement).value).toBe('/srv/orchbee/');
		fireEvent.keyDown(folderPath, { key: 'Escape' });
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
	environmentCatalog: ReturnType<
		DesktopSshProjectEnrollmentControllerClient['getEnvironmentCatalog']
	> = {
		discoveryIssue: null,
		managed: [],
		registryIssue: null,
		suggestions: [],
	},
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
		getEnvironmentCatalog: () => environmentCatalog,
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
