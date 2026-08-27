import type {
	DesktopBrowserAccessSnapshot,
	DesktopBrowserAccessState,
} from '@orchbee/desktop-client-protocol';
import { PersonalAccessTokenSchema } from '@orchbee/remote-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { DesktopBrowserAccessClientPort } from '../DesktopBrowserAccessClient';
import { DesktopBrowserAccessController } from '../DesktopBrowserAccessController';

describe('DesktopBrowserAccessController', () => {
	it('refreshes on open and starts loopback before opening the local Browser', async () => {
		const client = createClient();
		const controller = new DesktopBrowserAccessController(client);

		controller.open();
		await vi.waitFor(() => expect(controller.getSnapshot().pendingAction).toBeNull());
		controller.openLocalBrowser();
		await vi.waitFor(() => expect(controller.getSnapshot().pendingAction).toBeNull());

		expect(client.getState).toHaveBeenCalledOnce();
		expect(client.start).toHaveBeenCalledWith('loopback');
		expect(client.start.mock.invocationCallOrder[0]).toBeLessThan(
			client.openLocalBrowser.mock.invocationCallOrder[0] ?? Number.POSITIVE_INFINITY,
		);
		expect(controller.getSnapshot().snapshot.access).toMatchObject({
			mode: 'loopback',
			phase: 'ready',
		});
		controller.dispose();
	});

	it('keeps a bounded failure and current state available for localized rendering', async () => {
		const client = createClient();
		client.start.mockRejectedValueOnce(
			Object.assign(new Error('network address unavailable'), {
				code: 'network-address-unavailable' as const,
				snapshot: configuredSnapshot(),
			}),
		);
		const controller = new DesktopBrowserAccessController(client);

		controller.start('local-network');
		await vi.waitFor(() => expect(controller.getSnapshot().pendingAction).toBeNull());

		expect(controller.getSnapshot()).toMatchObject({
			error: 'network-address-unavailable',
			snapshot: { access: { phase: 'stopped' } },
		});
		controller.dispose();
	});

	it('records only the copied origin after Main performs the clipboard effect', async () => {
		const client = createClient({
			access: readyState('local-network'),
			personalAccessToken: 'configured',
		});
		const controller = new DesktopBrowserAccessController(client);

		controller.copyAccessLink('http://192.168.50.12:43110');
		await vi.waitFor(() => expect(controller.getSnapshot().pendingAction).toBeNull());

		expect(controller.getSnapshot().copiedOrigin).toBe('http://192.168.50.12:43110');
		expect(JSON.stringify(controller.getSnapshot())).not.toContain('#');
		controller.dispose();
	});

	it('creates, copies, and confirms reset of the Personal Access Token', async () => {
		const client = createClient({ access: { phase: 'stopped' }, personalAccessToken: 'missing' });
		const writeClipboardText = vi.fn<(text: string) => Promise<void>>(async () => undefined);
		const controller = new DesktopBrowserAccessController(client, writeClipboardText);

		controller.requestPersonalAccessTokenIssue();
		await vi.waitFor(() => expect(controller.getSnapshot().pendingAction).toBeNull());
		expect(controller.getSnapshot().issuedPersonalAccessToken).toBe('p'.repeat(43));

		controller.copyPersonalAccessToken();
		await vi.waitFor(() => expect(controller.getSnapshot().copiedPersonalAccessToken).toBe(true));
		expect(writeClipboardText).toHaveBeenCalledWith('p'.repeat(43));

		controller.requestPersonalAccessTokenIssue();
		expect(controller.getSnapshot().resetConfirmationOpen).toBe(true);
		controller.confirmPersonalAccessTokenReset();
		await vi.waitFor(() => expect(controller.getSnapshot().pendingAction).toBeNull());
		expect(client.issuePersonalAccessToken).toHaveBeenCalledTimes(2);
		controller.dispose();
		expect(controller.getSnapshot().issuedPersonalAccessToken).toBeNull();
	});
});

function createClient(initial: DesktopBrowserAccessSnapshot = configuredSnapshot()) {
	let snapshot = initial;
	const client = {
		copyAccessLink: vi.fn<DesktopBrowserAccessClientPort['copyAccessLink']>(async () => snapshot),
		dispose: vi.fn<() => void>(),
		getState: vi.fn<DesktopBrowserAccessClientPort['getState']>(async () => snapshot),
		issuePersonalAccessToken: vi.fn<DesktopBrowserAccessClientPort['issuePersonalAccessToken']>(
			async () => {
				snapshot = configuredSnapshot();
				return {
					personalAccessToken: PersonalAccessTokenSchema.parse('p'.repeat(43)),
					snapshot,
				};
			},
		),
		openLocalBrowser: vi.fn<DesktopBrowserAccessClientPort['openLocalBrowser']>(
			async () => snapshot,
		),
		start: vi.fn<DesktopBrowserAccessClientPort['start']>(async (mode) => {
			snapshot = { access: readyState(mode), personalAccessToken: 'configured' };
			return snapshot;
		}),
		stop: vi.fn<DesktopBrowserAccessClientPort['stop']>(async () => {
			snapshot = configuredSnapshot();
			return snapshot;
		}),
	} satisfies DesktopBrowserAccessClientPort;
	return client;
}

function configuredSnapshot(): DesktopBrowserAccessSnapshot {
	return { access: { phase: 'stopped' }, personalAccessToken: 'configured' };
}

function readyState(mode: 'local-network' | 'loopback'): DesktopBrowserAccessState {
	return {
		localOrigin: 'http://127.0.0.1:43110',
		mode,
		networkOrigins: mode === 'local-network' ? ['http://192.168.50.12:43110'] : [],
		phase: 'ready',
	};
}
