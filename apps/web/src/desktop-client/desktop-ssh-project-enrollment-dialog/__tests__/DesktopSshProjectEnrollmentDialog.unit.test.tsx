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
		{ destination: 'SSH destination', locale: 'en-US', title: 'Connect via SSH' },
		{ destination: 'SSH 目标', locale: 'zh-CN', title: '通过 SSH 连接' },
	] satisfies readonly { destination: string; locale: OrchbeeLocale; title: string }[])(
		'ships the enrollment form in $locale',
		({ destination, locale, title }) => {
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
			controller.dispose();
		},
	);

	it('moves through host verification and transient authentication before closing on commit', async () => {
		const responses: unknown[] = [];
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

		fireEvent.change(screen.getByLabelText('Environment name'), {
			target: { value: 'Build machine' },
		});
		fireEvent.change(screen.getByLabelText('SSH destination'), {
			target: { value: 'build-host' },
		});
		fireEvent.change(screen.getByLabelText('Remote Project path'), {
			target: { value: '/srv/orchbee' },
		});
		fireEvent.click(screen.getByRole('button', { name: 'Connect' }));

		await screen.findByRole('heading', { name: 'Verify SSH Host' });
		expect(screen.getByText('The authenticity of host cannot be established.')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Continue' }));

		await screen.findByRole('heading', { name: 'SSH Authentication Required' });
		const response = screen.getByLabelText('Response');
		expect(response.getAttribute('type')).toBe('password');
		fireEvent.change(response, { target: { value: 'one-use secret' } });
		expect(JSON.stringify(controller.getDialogSnapshot())).not.toContain('one-use secret');
		fireEvent.click(screen.getByRole('button', { name: 'Continue' }));

		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(responses).toEqual([
			{ kind: 'submitted', response: 'yes' },
			{ kind: 'submitted', response: 'one-use secret' },
		]);
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
		connect: async (projectId) => ({ kind: 'connected', projectId }),
		disconnect: async (projectId) => ({ kind: 'disconnected', projectId }),
		dispose: () => undefined,
		enroll,
		getStatuses: () => [],
		subscribeStatuses: () => () => undefined,
	};
	return new DesktopSshProjectEnrollmentController({ client, onEnrollmentCommitted });
}
