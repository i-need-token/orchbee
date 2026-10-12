import {
	DesktopUserPreferencesCommandSchema,
	DesktopUserPreferencesEventSchema,
	type DesktopUserPreferencesCommand,
} from '@orchbee/desktop-client-protocol';
import {
	DEFAULT_USER_PREFERENCES,
	UserPreferencesSnapshotSchema,
	type UserPreferencesResponse,
} from '@orchbee/workbench-client-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { DesktopUserPreferencesClient } from '../DesktopUserPreferencesClient';
import { ApplicationLocaleController } from '../../../application-locale';
import { TerminalSettingsController } from '../../../terminal-settings';
import { TerminalInteractionSettingsController } from '../../../terminal-interaction-settings';
import { TerminalFeedbackSettingsController } from '../../../terminal-feedback-settings';

const cleanup: (() => void)[] = [];
afterEach(() => {
	for (const dispose of cleanup.splice(0)) dispose();
});
const initial = UserPreferencesSnapshotSchema.parse({
	source: 'a'.repeat(64),
	values: DEFAULT_USER_PREFERENCES,
	claimed: [],
	desktopImported: true,
	desktopMainImported: true,
});

function fixture() {
	const channel = new MessageChannel();
	const commands: DesktopUserPreferencesCommand[] = [];
	channel.port2.addEventListener('message', (event: MessageEvent<unknown>) => {
		commands.push(DesktopUserPreferencesCommandSchema.parse(event.data));
	});
	channel.port2.start();
	const client = new DesktopUserPreferencesClient(channel.port1);
	cleanup.push(() => {
		client.dispose();
		channel.port2.close();
	});
	const reply = (requestId: number | null, response: UserPreferencesResponse): void => {
		channel.port2.postMessage(
			DesktopUserPreferencesEventSchema.parse({
				type: 'desktop-user-preferences-observed',
				requestId,
				response,
				applicationIssue: null,
			}),
		);
	};
	return { client, commands, reply, peer: channel.port2 };
}

async function initialize(test: ReturnType<typeof fixture>): Promise<void> {
	const ready = test.client.initialize(() => ({}));
	await vi.waitFor(() => expect(test.commands).toHaveLength(1));
	test.reply(1, { kind: 'snapshot', snapshot: initial, changed: false });
	await ready;
}

describe('Desktop common preference consumer', () => {
	it('clears an initial read refusal after a confirmed poll without fabricating a failed save', async () => {
		const test = fixture();
		const ready = test.client.initialize(() => ({}));
		await vi.waitFor(() => expect(test.commands).toHaveLength(1));
		test.reply(1, { kind: 'refused', reason: 'busy' });
		await ready;
		expect(test.client.getSnapshot()).toMatchObject({ available: false, failure: 'busy' });
		test.reply(null, { kind: 'snapshot', snapshot: initial, changed: false });
		await vi.waitFor(() =>
			expect(test.client.getSnapshot()).toMatchObject({ available: true, failure: null }),
		);
	});
	it('publishes all renderer behaviors only after confirmation and never writes common browser records', async () => {
		const test = fixture();
		await initialize(test);
		const storage = { getItem: () => null, setItem: vi.fn<(key: string, value: string) => void>() };
		const documentElement = { dir: 'ltr', lang: 'en-US', dataset: {} };
		const locale = new ApplicationLocaleController({
			storage,
			documentElement,
			preferredLocales: ['en-US'],
			userPreferences: test.client,
		});
		const appearance = new TerminalSettingsController({
			storage,
			themeTarget: documentElement,
			userPreferences: test.client,
		});
		const interaction = new TerminalInteractionSettingsController(storage, test.client);
		const feedback = new TerminalFeedbackSettingsController({
			storage,
			userPreferences: test.client,
			notificationCapability: {
				permission: () => 'granted',
				requestPermission: async () => 'granted',
				show: () => true,
			},
		});
		cleanup.push(() => {
			locale.dispose();
			appearance.dispose();
			interaction.dispose();
			feedback.dispose();
		});
		expect(appearance.update({ applicationTheme: 'dark' }).kind).toBe('pending');
		await vi.waitFor(() => expect(test.commands).toHaveLength(2));
		expect(documentElement.dataset).toEqual({ theme: 'light' });
		expect(test.commands[1]?.request).toEqual({
			operation: 'update',
			source: initial.source,
			changes: { applicationTheme: 'dark' },
		});
		test.reply(2, { kind: 'refused', reason: 'write-failed' });
		await vi.waitFor(() => expect(test.client.getSnapshot().failure).toBe('write-failed'));
		expect(documentElement.dataset).toEqual({ theme: 'light' });
		const snapshot = UserPreferencesSnapshotSchema.parse({
			...initial,
			source: 'b'.repeat(64),
			values: {
				...initial.values,
				applicationTheme: 'dark',
				locale: 'zh-CN',
				copyOnSelect: false,
				trimTrailingBlankRowsForAgents: false,
				agentNotificationMode: 'hidden',
				completionReminderMode: 'always',
				completionReminderMinimumSeconds: 17,
				soundEnabled: true,
			},
		});
		test.reply(null, { kind: 'snapshot', snapshot, changed: false });
		await vi.waitFor(() => expect(documentElement.lang).toBe('zh-CN'));
		expect(documentElement.dataset).toEqual({ theme: 'dark' });
		expect(interaction.getSnapshot().preferences).toEqual({
			copyOnSelect: false,
			trimTrailingBlankRowsForAgents: false,
		});
		expect(feedback.getSnapshot().preferences).toEqual({
			agentNotificationMode: 'hidden',
			completionReminderMode: 'always',
			completionReminderMinimumSeconds: 17,
			soundEnabled: true,
		});
		expect(test.client.getSnapshot().failure).toBe('write-failed');
		expect(storage.setItem).not.toHaveBeenCalled();
	});

	it('keeps an unknown write visible through polling and clears it only after an explicit read', async () => {
		const test = fixture();
		await initialize(test);
		const result = test.client.reset(['copyOnSelect']);
		await vi.waitFor(() => expect(test.commands).toHaveLength(2));
		test.reply(2, { kind: 'outcome-unknown' });
		expect(await result).toEqual({ kind: 'outcome-unknown' });
		test.reply(null, { kind: 'snapshot', snapshot: initial, changed: false });
		await vi.waitFor(() => expect(test.client.getSnapshot().available).toBe(true));
		expect(test.client.getSnapshot().failure).toBe('outcome-unknown');
		expect(test.commands).toHaveLength(2);
		const read = test.client.refresh();
		await vi.waitFor(() => expect(test.commands).toHaveLength(3));
		test.reply(3, { kind: 'snapshot', snapshot: initial, changed: false });
		await read;
		expect(test.client.getSnapshot()).toMatchObject({ failure: null, draftResetRevision: 1 });
	});

	it('retires a malformed response with an uncertain save, without resubmission', async () => {
		const test = fixture();
		await initialize(test);
		const pending = test.client.update({ copyOnSelect: false });
		await vi.waitFor(() => expect(test.commands).toHaveLength(2));
		test.peer.postMessage({ type: 'desktop-user-preferences-observed', requestId: 99 });
		expect(await pending).toEqual({ kind: 'outcome-unknown' });
		expect(test.commands).toHaveLength(2);
		expect(test.client.getSnapshot()).toMatchObject({
			available: false,
			failure: 'outcome-unknown',
		});
	});
});
