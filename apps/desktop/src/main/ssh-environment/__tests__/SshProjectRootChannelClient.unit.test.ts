import {
	SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
	SSH_PROJECT_ROOT_SERVICE_ID,
	SshEnvironmentChannelIdSchema,
	SshEnvironmentRuntimeEpochSchema,
	SshProjectRootDirectoryListingResultSchema,
	SshProjectRootInspectionResultSchema,
	decodeSshEnvironmentFrame,
	decodeSshProjectRootDirectoryListingRequest,
	decodeSshProjectRootInspectionRequest,
	encodeSshProjectRootDirectoryListingResult,
	encodeSshProjectRootInspectionResult,
	type SshEnvironmentChannelId,
	type SshEnvironmentMessage,
} from '@orchbee/ssh-environment-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { SshProjectRootChannelClient } from '../SshProjectRootChannelClient';

const runtimeEpoch = SshEnvironmentRuntimeEpochSchema.parse('2'.repeat(32));

afterEach(() => vi.useRealTimers());

describe('SSH project-root channel client', () => {
	it('runs the exact open, request, result, close, and closed lifecycle', async () => {
		const harness = createHarness();
		const inspection = harness.client.inspect('/srv/project-link');
		const channelId = requireOpenedChannelId(harness.messages[0]);
		expect(harness.messages[0]).toEqual({
			type: 'channel-open',
			payload: {
				runtimeEpoch,
				channelId,
				serviceId: SSH_PROJECT_ROOT_SERVICE_ID,
				initialCreditBytes: SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
			},
		});

		expect(harness.client.handle(opened(channelId))).toBe('handled');
		const requestMessage = harness.messages[1];
		expect(requestMessage?.type).toBe('channel-data');
		if (requestMessage?.type !== 'channel-data') throw new Error('missing project-root request');
		expect(decodeSshProjectRootInspectionRequest(requestMessage.data)).toEqual({
			ok: true,
			message: { type: 'inspect-project-root', path: '/srv/project-link' },
		});

		const result = SshProjectRootInspectionResultSchema.parse({
			kind: 'resolved',
			canonicalPath: '/srv/project',
		});
		expect(
			harness.client.handle({
				type: 'channel-data',
				channelId,
				data: encodeSshProjectRootInspectionResult(result),
			}),
		).toBe('handled');
		await expect(inspection).resolves.toEqual(result);
		expect(harness.messages[2]).toEqual({
			type: 'channel-close',
			payload: { runtimeEpoch, channelId, reason: 'completed' },
		});
		expect(harness.client.handle(closed(channelId, 'completed'))).toBe('handled');
		expect(harness.onAttachmentInvalid).not.toHaveBeenCalled();
	});

	it('lists the remote home or an absolute directory through the same bounded lifecycle', async () => {
		const harness = createHarness();
		const listing = harness.client.list(null);
		const channelId = requireOpenedChannelId(harness.messages[0]);
		expect(harness.client.handle(opened(channelId))).toBe('handled');
		const requestMessage = harness.messages[1];
		if (requestMessage?.type !== 'channel-data') throw new Error('missing directory request');
		expect(decodeSshProjectRootDirectoryListingRequest(requestMessage.data)).toEqual({
			ok: true,
			message: { type: 'list-project-directory', path: null },
		});

		const result = SshProjectRootDirectoryListingResultSchema.parse({
			kind: 'listed',
			canonicalPath: '/home/alice',
			parentPath: '/home',
			directories: [{ name: '.config' }, { name: 'src' }],
			truncated: false,
		});
		expect(
			harness.client.handle({
				type: 'channel-data',
				channelId,
				data: encodeSshProjectRootDirectoryListingResult(result),
			}),
		).toBe('handled');
		await expect(listing).resolves.toEqual(result);
		expect(harness.client.handle(closed(channelId, 'completed'))).toBe('handled');
	});

	it('bounds concurrency, cancels without sending a request, and allocates a fresh channel', async () => {
		const harness = createHarness();
		const controllers = Array.from({ length: 4 }, () => new AbortController());
		const active = controllers.map((controller, index) =>
			harness.client.inspect(`/srv/project-${index}`, controller.signal),
		);
		await expect(harness.client.inspect('/srv/project-over-capacity')).resolves.toEqual({
			kind: 'failed',
			reason: 'busy',
		});

		const firstChannelId = requireOpenedChannelId(harness.messages[0]);
		controllers[0]?.abort();
		await expect(active[0]).resolves.toEqual({ kind: 'cancelled' });
		expect(harness.client.handle(opened(firstChannelId))).toBe('handled');
		expect(harness.client.handle(closed(firstChannelId, 'cancelled'))).toBe('handled');

		void harness.client.inspect('/srv/fresh-project');
		const freshOpen = harness.messages.at(-1);
		const freshChannelId = requireOpenedChannelId(freshOpen);
		expect(freshChannelId).not.toBe(firstChannelId);
		harness.client.retire();
		await expect(Promise.all(active.slice(1))).resolves.toEqual([
			{ kind: 'failed', reason: 'unavailable' },
			{ kind: 'failed', reason: 'unavailable' },
			{ kind: 'failed', reason: 'unavailable' },
		]);
	});

	it('settles at the caller deadline and never waits for channel acknowledgement', async () => {
		vi.useFakeTimers();
		const harness = createHarness();
		const inspection = harness.client.inspect('/srv/slow-project');
		const channelId = requireOpenedChannelId(harness.messages[0]);

		await vi.advanceTimersByTimeAsync(30_000);
		await expect(inspection).resolves.toEqual({ kind: 'failed', reason: 'timed-out' });
		expect(harness.messages.at(-1)).toEqual({
			type: 'channel-close',
			payload: { runtimeEpoch, channelId, reason: 'cancelled' },
		});
		harness.client.retire();
	});

	it('contains invalid service payloads to their channel and retires on channel corruption', async () => {
		const harness = createHarness();
		const inspection = harness.client.inspect('/srv/project');
		const channelId = requireOpenedChannelId(harness.messages[0]);
		expect(harness.client.handle(opened(channelId))).toBe('handled');
		expect(
			harness.client.handle({
				type: 'channel-data',
				channelId,
				data: new TextEncoder().encode('{}'),
			}),
		).toBe('handled');
		await expect(inspection).resolves.toEqual({ kind: 'failed', reason: 'unavailable' });
		expect(harness.messages.at(-1)).toEqual({
			type: 'channel-close',
			payload: { runtimeEpoch, channelId, reason: 'protocol-failure' },
		});
		expect(harness.client.handle(closed(channelId, 'protocol-failure'))).toBe('handled');

		const unknownChannel = SshEnvironmentChannelIdSchema.parse(4_294_967_295);
		expect(
			harness.client.handle({
				type: 'channel-data',
				channelId: unknownChannel,
				data: new Uint8Array(),
			}),
		).toBe('unhandled');
	});

	it('rejects invalid local paths and fences every pending invocation on attachment loss', async () => {
		const harness = createHarness();
		await expect(harness.client.inspect('relative/project')).resolves.toEqual({
			kind: 'failed',
			reason: 'invalid-path',
		});
		expect(harness.messages).toHaveLength(0);

		const first = harness.client.inspect('/srv/first');
		const second = harness.client.inspect('/srv/second');
		harness.client.retire();
		await expect(first).resolves.toEqual({ kind: 'failed', reason: 'unavailable' });
		await expect(second).resolves.toEqual({ kind: 'failed', reason: 'unavailable' });
		await expect(harness.client.inspect('/srv/not-replayed')).resolves.toEqual({
			kind: 'failed',
			reason: 'unavailable',
		});
		expect(harness.messages).toHaveLength(2);
	});
});

function createHarness() {
	const messages: SshEnvironmentMessage[] = [];
	const onAttachmentInvalid = vi.fn<() => void>();
	let nextChannelId = 1;
	const client = new SshProjectRootChannelClient({
		allocateChannelId: () => SshEnvironmentChannelIdSchema.parse(nextChannelId++),
		onAttachmentInvalid,
		runtimeEpoch,
		write: async (bytes) => {
			const decoded = decodeSshEnvironmentFrame(bytes);
			if (!decoded.ok) throw new Error(decoded.error.code);
			messages.push(decoded.message);
		},
	});
	return { client, messages, onAttachmentInvalid };
}

function requireOpenedChannelId(
	message: SshEnvironmentMessage | undefined,
): SshEnvironmentChannelId {
	if (message?.type !== 'channel-open') throw new Error('missing channel-open message');
	return message.payload.channelId;
}

function opened(channelId: SshEnvironmentChannelId): SshEnvironmentMessage {
	return {
		type: 'channel-opened',
		payload: {
			runtimeEpoch,
			channelId,
			initialCreditBytes: SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
		},
	};
}

function closed(
	channelId: SshEnvironmentChannelId,
	reason: 'cancelled' | 'completed' | 'protocol-failure',
): SshEnvironmentMessage {
	return { type: 'channel-closed', payload: { runtimeEpoch, channelId, reason } };
}
