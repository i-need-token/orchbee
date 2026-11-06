import { DEFAULT_USER_PREFERENCES, USER_PREFERENCE_KEYS } from '@orchbee/workbench-client-protocol';
import { expect, it, onTestFinished } from 'vitest';
import { ServerUserPreferencesClient } from '../ServerUserPreferencesClient';

it('shows recovery defaults and sends only confirmed Reset all after an invalid-source read', async () => {
	const operations: unknown[] = [];
	const client = new ServerUserPreferencesClient(
		{
			exchange: async (record) => {
				const request = JSON.parse(new TextDecoder().decode(record.subarray(12))) as {
					requestId: string;
					sessionId: string;
					operation: { kind: string; source?: string; changes?: unknown };
				};
				operations.push(request.operation);
				const outcome =
					request.operation.kind === 'open'
						? { kind: 'refused', reason: 'invalid-source' }
						: {
								kind: 'saved',
								changed: true,
								snapshot: {
									source: 'a'.repeat(64),
									values: DEFAULT_USER_PREFERENCES,
									claimed: USER_PREFERENCE_KEYS,
								},
							};
				const body = new TextEncoder().encode(
					JSON.stringify({
						kind: 'workbench.settings.result',
						protocolVersion: 1,
						requestId: request.requestId,
						sessionId: request.sessionId,
						outcome,
					}),
				);
				const response = new Uint8Array(12 + body.length);
				response.set([0x4f, 0x42, 0x57, 0x50]);
				const view = new DataView(response.buffer);
				view.setUint16(4, 1);
				view.setUint16(6, 60);
				view.setUint32(8, body.length);
				response.set(body, 12);
				return response;
			},
			subscribe: () => ({ close: () => {}, resnapshot: () => {} }),
		},
		'1'.repeat(32),
	);
	onTestFinished(() => client.dispose());
	await client.refresh();
	expect(client.getSnapshot()).toMatchObject({
		available: false,
		failure: 'invalid-source',
		snapshot: { source: 'unreadable', values: DEFAULT_USER_PREFERENCES },
	});
	await client.update({ applicationTheme: 'dark' });
	await client.reset(['applicationTheme']);
	expect(operations).toHaveLength(1);
	expect(client.getSnapshot().failure).toBe('invalid-source');
	expect((await client.reset(USER_PREFERENCE_KEYS)).kind).toBe('snapshot');
	expect(operations).toEqual([
		{ kind: 'open' },
		{
			kind: 'save',
			source: 'unreadable',
			changes: Object.fromEntries(USER_PREFERENCE_KEYS.map((key) => [key, null])),
		},
	]);
	expect(client.getSnapshot()).toMatchObject({
		available: true,
		failure: null,
		snapshot: { source: 'a'.repeat(64) },
	});
});
