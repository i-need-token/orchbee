import {
	UserPreferencesResponseSchema,
	DEFAULT_USER_PREFERENCES,
	type UserPreferencesResponse,
} from '@orchbee/workbench-client-protocol';
import {
	DesktopUserPreferencesCommandSchema,
	DesktopUserPreferencesEventSchema,
} from '@orchbee/desktop-client-protocol';

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
const initial = UserPreferencesResponseSchema.options[0].shape.snapshot.parse({
	source: 'a'.repeat(64),
	values: DEFAULT_USER_PREFERENCES,
	claimed: [],
});

function fixture() {
	const channel = new MessageChannel();
	const commands: ReturnType<typeof DesktopUserPreferencesCommandSchema.parse>[] = [];
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
	const ready = test.client.refresh();
	await vi.waitFor(() => expect(test.commands).toHaveLength(1));
	test.reply(1, { kind: 'snapshot', snapshot: initial, changed: false });
	await ready;
}

function appearanceFixture(client: DesktopUserPreferencesClient) {
	const storage = { getItem: () => null, setItem: vi.fn<(key: string, value: string) => void>() };
	const themeTarget = { dataset: {} };
	const appearance = new TerminalSettingsController({
		storage,
		themeTarget,
		userPreferences: client,
	});
	cleanup.push(() => appearance.dispose());
	return { appearance, storage, themeTarget };
}

describe('Desktop common preference consumer', () => {
	it.each(['appearance', 'interaction', 'feedback'] as const)(
		'rejects %s Reset after its local controller is disposed while the shared client remains live',
		async (owner) => {
			const test = fixture();
			await initialize(test);
			const { appearance, storage } = appearanceFixture(test.client);
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
			cleanup.push(
				() => interaction.dispose(),
				() => feedback.dispose(),
			);
			const controller = { appearance, interaction, feedback }[owner];
			controller.dispose();
			const before = controller.getSnapshot();
			const sharedBefore = test.client.getSnapshot();
			const reset = vi.spyOn(test.client, 'reset');

			expect(controller.reset()).toEqual({ kind: 'disposed' });
			expect(reset).not.toHaveBeenCalled();
			expect(controller.getSnapshot()).toBe(before);
			expect(test.client.getSnapshot()).toBe(sharedBefore);
			expect(storage.setItem).not.toHaveBeenCalled();
		},
	);

	it('does not start a common Reset after host-reset publication disposes the controller', async () => {
		const test = fixture();
		await initialize(test);
		const { appearance } = appearanceFixture(test.client);
		appearance.update({ fontSize: 14 });
		appearance.subscribeSnapshot(() => appearance.dispose());
		const reset = vi.spyOn(test.client, 'reset');
		const before = test.client.getSnapshot();

		expect(appearance.reset()).toEqual({ kind: 'disposed' });
		expect(appearance.current().fontSize).toBe(13);
		expect(reset).not.toHaveBeenCalled();
		expect(test.client.getSnapshot()).toBe(before);
	});

	it('preserves a newer host edit made during common-write admission and later common confirmation', async () => {
		const test = fixture();
		await initialize(test);
		const { appearance, storage, themeTarget } = appearanceFixture(test.client);
		const remove = test.client.subscribe(() => {
			if (!test.client.getSnapshot().pending) return;
			remove();
			appearance.update({ fontSize: 15 });
		});

		expect(appearance.update({ applicationTheme: 'dark', fontSize: 14 })).toEqual({
			kind: 'pending',
		});
		expect(appearance.current().fontSize).toBe(15);
		expect(themeTarget.dataset).toEqual({ theme: 'light' });
		expect(storage.setItem).toHaveBeenCalledExactlyOnceWith(
			'orchbee:web:terminal-host-appearance:v1',
			expect.stringContaining('"fontSize":15'),
		);
		await vi.waitFor(() => expect(test.commands).toHaveLength(2));
		expect(test.commands[1]?.request).toEqual({
			operation: 'update',
			source: initial.source,
			changes: { applicationTheme: 'dark' },
		});
		test.reply(2, {
			kind: 'snapshot',
			changed: true,
			snapshot: UserPreferencesResponseSchema.options[0].shape.snapshot.parse({
				...initial,
				source: 'b'.repeat(64),
				claimed: ['applicationTheme'],
				values: { ...initial.values, applicationTheme: 'dark' },
			}),
		});
		await vi.waitFor(() => expect(themeTarget.dataset).toEqual({ theme: 'dark' }));
		expect(appearance.current().fontSize).toBe(15);
		expect(storage.setItem).toHaveBeenCalledOnce();
	});

	it('retires an appearance candidate during admission without cancelling the already admitted common write', async () => {
		const test = fixture();
		await initialize(test);
		const { appearance, storage, themeTarget } = appearanceFixture(test.client);
		const before = appearance.getSnapshot();
		const remove = test.client.subscribe(() => {
			if (!test.client.getSnapshot().pending) return;
			remove();
			appearance.dispose();
		});

		expect(appearance.update({ applicationTheme: 'dark', fontSize: 14 })).toEqual({
			kind: 'disposed',
		});
		expect(appearance.getSnapshot()).toBe(before);
		expect(appearance.current().fontSize).toBe(13);
		expect(storage.setItem).not.toHaveBeenCalled();
		await vi.waitFor(() => expect(test.commands).toHaveLength(2));
		test.reply(2, {
			kind: 'snapshot',
			changed: true,
			snapshot: UserPreferencesResponseSchema.options[0].shape.snapshot.parse({
				...initial,
				source: 'b'.repeat(64),
				claimed: ['applicationTheme'],
				values: { ...initial.values, applicationTheme: 'dark' },
			}),
		});
		await vi.waitFor(() =>
			expect(test.client.getSnapshot().snapshot?.values.applicationTheme).toBe('dark'),
		);
		expect(appearance.getSnapshot()).toBe(before);
		expect(themeTarget.dataset).toEqual({ theme: 'light' });
		expect(storage.setItem).not.toHaveBeenCalled();
	});

	it('does not reset drafts after a refresh receipt retires the client synchronously', async () => {
		const test = fixture();
		await initialize(test);
		const initialResetRevision = test.client.getSnapshot().draftResetRevision;
		let retired: ReturnType<DesktopUserPreferencesClient['getSnapshot']> | undefined;
		test.client.subscribe(() => {
			const state = test.client.getSnapshot();
			if (state.pending || !state.available) return;
			test.client.dispose();
			retired = test.client.getSnapshot();
		});
		const refreshed = test.client.refresh();
		await vi.waitFor(() => expect(test.commands).toHaveLength(2));
		test.reply(2, { kind: 'snapshot', snapshot: initial, changed: false });
		expect((await refreshed).kind).toBe('snapshot');
		expect(retired).toBeDefined();
		expect(test.client.getSnapshot()).toBe(retired);
		expect(test.client.getSnapshot().draftResetRevision).toBe(initialResetRevision);
	});

	it('does not repeat a subscriber re-registered during its current notification', async () => {
		const test = fixture();
		await initialize(test);
		let notifications = 0;
		const listener = (): void => {
			notifications += 1;
			if (notifications === 1) {
				unsubscribe();
				unsubscribe = test.client.subscribe(listener);
			}
		};
		let unsubscribe = test.client.subscribe(listener);
		const pending = test.client.update({ copyOnSelect: false });
		expect(notifications).toBe(1);
		await vi.waitFor(() => expect(test.commands).toHaveLength(2));
		test.reply(2, { kind: 'refused', reason: 'busy' });
		expect(await pending).toEqual({ kind: 'refused', reason: 'busy' });
		expect(notifications).toBe(2);
	});

	it('skips a subscriber removed before its current notification', async () => {
		const test = fixture();
		await initialize(test);
		const listener = vi.fn<() => void>();
		test.client.subscribe(() => {
			if (test.client.getSnapshot().pending) remove();
		});
		const remove = test.client.subscribe(listener);
		const pending = test.client.update({ copyOnSelect: false });
		expect(listener).not.toHaveBeenCalled();
		await vi.waitFor(() => expect(test.commands).toHaveLength(2));
		test.reply(2, { kind: 'refused', reason: 'busy' });
		expect(await pending).toEqual({ kind: 'refused', reason: 'busy' });
		expect(listener).not.toHaveBeenCalled();
	});

	it('serializes renderer-local appearance writes without sending common preference commands', async () => {
		const test = fixture();
		await initialize(test);
		const update = vi.spyOn(test.client, 'update');
		let nestedResult: unknown;
		let stored = '';
		let writes = 0;
		const storage = {
			getItem: () => null,
			setItem: vi.fn<(key: string, value: string) => void>((_key, value) => {
				writes += 1;
				if (writes === 1) {
					nestedResult = appearance.update({ fontSize: 15 });
				}
				stored = value;
			}),
		};
		const appearance = new TerminalSettingsController({
			storage,
			themeTarget: { dataset: {} },
			userPreferences: test.client,
		});
		cleanup.push(() => appearance.dispose());
		expect(appearance.update({ fontSize: 14 })).toEqual({ kind: 'applied-and-saved' });
		expect(nestedResult).toEqual({ kind: 'applied-session-only' });
		expect(appearance.current().fontSize).toBe(15);
		expect(storage.setItem.mock.calls.map(([key]) => key)).toEqual([
			'orchbee:web:terminal-host-appearance:v1',
			'orchbee:web:terminal-host-appearance:v1',
		]);
		expect(JSON.parse(stored)).toEqual({
			backgroundOpacityPercent: 100,
			fontFamily: appearance.current().fontFamily,
			fontLigatures: true,
			fontSize: 15,
			fontWeight: 400,
			fontWeightBold: 700,
			terminalPadding: 0,
		});
		expect(update).not.toHaveBeenCalled();
	});

	it('clears an initial read refusal after a confirmed poll without fabricating a failed save', async () => {
		const test = fixture();
		const ready = test.client.refresh();
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
		const snapshot = UserPreferencesResponseSchema.options[0].shape.snapshot.parse({
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
		const initialResetRevision = test.client.getSnapshot().draftResetRevision;
		const result = test.client.reset(['copyOnSelect']);
		await vi.waitFor(() => expect(test.commands).toHaveLength(2));
		test.reply(2, { kind: 'outcome-unknown' });
		expect(await result).toEqual({ kind: 'outcome-unknown' });
		test.reply(null, { kind: 'snapshot', snapshot: initial, changed: false });
		await vi.waitFor(() => expect(test.client.getSnapshot().available).toBe(true));
		expect(test.client.getSnapshot().failure).toBe('outcome-unknown');
		expect(test.client.getSnapshot().draftResetRevision).toBe(initialResetRevision);
		expect(test.commands).toHaveLength(2);
		const read = test.client.refresh();
		await vi.waitFor(() => expect(test.commands).toHaveLength(3));
		test.reply(3, { kind: 'snapshot', snapshot: initial, changed: false });
		await read;
		expect(test.client.getSnapshot()).toMatchObject({
			failure: null,
			draftResetRevision: initialResetRevision + 1,
		});
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
		const retired = test.client.getSnapshot();
		const listener = vi.fn<() => void>();
		test.client.subscribe(listener);
		expect(await test.client.update({ copyOnSelect: true })).toEqual({
			kind: 'refused',
			reason: 'unavailable',
		});
		expect(await test.client.reset(['copyOnSelect'])).toEqual({
			kind: 'refused',
			reason: 'unavailable',
		});
		expect(await test.client.refresh()).toEqual({ kind: 'refused', reason: 'unavailable' });
		expect(test.client.getSnapshot()).toBe(retired);
		expect(listener).not.toHaveBeenCalled();
		expect(test.commands).toHaveLength(2);
	});
});
