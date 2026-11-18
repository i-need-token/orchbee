import {
	DesktopRelaunchPlaceSchema,
	type DesktopRelaunchPlace,
	type DesktopWindowUpdateMainMessage,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { ServerStartAnswer } from '../../server-connection';
import { DesktopWindowUpdate, type DesktopWindowUpdatePort } from '../DesktopWindowUpdate';

const ROOT = 'C:\\Programs\\Orchbee\\desktop';
const PLACE = DesktopRelaunchPlaceSchema.parse({
	kind: 'workspace',
	workspaceId: '0123456789abcdef0123456789abcdef',
});

function fakePort() {
	let receive: ((event: { readonly data: unknown }) => void) | undefined;
	const sent: DesktopWindowUpdateMainMessage[] = [];
	const port: DesktopWindowUpdatePort = {
		close: vi.fn<() => void>(),
		on: (_event, listener) => {
			receive = listener;
		},
		postMessage: (message) => sent.push(message),
		start: vi.fn<() => void>(),
	};
	return {
		port,
		request: (version: string, place: DesktopRelaunchPlace | null = null) =>
			receive?.({ data: { place, type: 'desktop-window-update-requested', version } }),
		sent,
	};
}

function started(negotiatedCapabilities: readonly string[]): ServerStartAnswer {
	return { negotiatedCapabilities, restore: { kind: 'intact' } };
}

function create(options: {
	readonly active: string | null;
	readonly startVersionClient?: (root: string, version: string) => Promise<ServerStartAnswer>;
}) {
	let active = options.active;
	let changed: (() => void) | undefined;
	const onJoinable = vi.fn<(version: string, place: DesktopRelaunchPlace | null) => Promise<void>>(
		() => Promise.resolve(),
	);
	const startVersionClient = vi.fn<(root: string, version: string) => Promise<ServerStartAnswer>>(
		options.startVersionClient ?? (() => Promise.resolve(started([]))),
	);
	const update = new DesktopWindowUpdate({
		desktopRoot: ROOT,
		fallbackFrom: null,
		onJoinable,
		ownVersion: '0.2.0',
		readPointer: () => Promise.resolve(active),
		recordEvent: () => undefined,
		startVersionClient,
		watchPointer: (_root, onChange) => {
			changed = onChange;
			return () => undefined;
		},
	});
	update.start();
	return {
		movePointer: (version: string | null) => {
			active = version;
			changed?.();
		},
		onJoinable,
		startVersionClient,
		update,
	};
}

async function settle(): Promise<void> {
	await new Promise((resolve) => setTimeout(resolve, 0));
}

describe('DesktopWindowUpdate', () => {
	it('offers each document the newer window version, and follows the pointer as it moves', async () => {
		const { movePointer, update } = create({ active: '0.3.0' });
		await settle();
		const document = fakePort();
		update.serve(document.port);
		expect(document.sent).toEqual([{ type: 'desktop-window-update-offered', version: '0.3.0' }]);
		movePointer('0.2.0');
		await settle();
		expect(document.sent.at(-1)).toEqual({ type: 'desktop-window-update-offered', version: null });
	});

	it('says the update waits when the new version cannot join this Session', async () => {
		const { onJoinable, startVersionClient, update } = create({
			active: '0.3.0',
			startVersionClient: () => Promise.resolve(started(['agent-input-area'])),
		});
		await settle();
		const document = fakePort();
		update.serve(document.port);
		document.request('0.3.0');
		await settle();
		expect(startVersionClient).toHaveBeenCalledWith(ROOT, '0.3.0');
		expect(document.sent.at(-1)).toEqual({ type: 'desktop-window-update-waiting' });
		expect(onJoinable).not.toHaveBeenCalled();
	});

	it('switches only when the new version negotiates the compatibility window', async () => {
		const { onJoinable, update } = create({
			active: '0.3.0',
			startVersionClient: () => Promise.resolve(started(['compatibility-window'])),
		});
		await settle();
		const document = fakePort();
		update.serve(document.port);
		document.request('0.3.0', PLACE);
		await settle();
		expect(onJoinable).toHaveBeenCalledWith('0.3.0', PLACE);
		expect(document.sent).not.toContainEqual({ type: 'desktop-window-update-waiting' });
	});

	it('ignores a request for anything but the current offer, and one made while a check runs', async () => {
		let finish: ((answer: ServerStartAnswer) => void) | undefined;
		const { startVersionClient, update } = create({
			active: '0.3.0',
			startVersionClient: () =>
				new Promise((resolve) => {
					finish = resolve;
				}),
		});
		await settle();
		const document = fakePort();
		update.serve(document.port);
		document.request('0.2.5');
		document.request('0.3.0');
		document.request('0.3.0');
		expect(startVersionClient).toHaveBeenCalledOnce();
		finish?.(started([]));
		await settle();
		expect(
			document.sent.filter((message) => message.type === 'desktop-window-update-waiting'),
		).toHaveLength(1);
	});

	it('follows a window store that appears after start once a document is served', async () => {
		let active: string | null = null;
		let changed: (() => void) | undefined;
		let watchable = false;
		const update = new DesktopWindowUpdate({
			desktopRoot: ROOT,
			fallbackFrom: null,
			onJoinable: () => Promise.resolve(),
			ownVersion: '0.2.0',
			readPointer: () => Promise.resolve(active),
			recordEvent: () => undefined,
			startVersionClient: () => Promise.resolve(started([])),
			watchPointer: (_root, onChange) => {
				if (!watchable) return null;
				changed = onChange;
				return () => undefined;
			},
		});
		update.start();
		await settle();
		watchable = true;
		const document = fakePort();
		update.serve(document.port);
		active = '0.3.0';
		changed?.();
		await settle();
		expect(document.sent.at(-1)).toEqual({
			type: 'desktop-window-update-offered',
			version: '0.3.0',
		});
	});

	it('offers nothing and checks nothing outside a version folder', async () => {
		const readPointer = vi.fn<(root: string) => Promise<string | null>>(() =>
			Promise.resolve('0.3.0'),
		);
		const update = new DesktopWindowUpdate({
			desktopRoot: null,
			fallbackFrom: null,
			onJoinable: () => Promise.resolve(),
			ownVersion: '0.2.0',
			readPointer,
			recordEvent: () => undefined,
			startVersionClient: () => Promise.resolve(started(['compatibility-window'])),
			watchPointer: () => () => undefined,
		});
		update.start();
		await settle();
		const document = fakePort();
		update.serve(document.port);
		document.request('0.3.0');
		await settle();
		expect(readPointer).not.toHaveBeenCalled();
		expect(document.sent).toEqual([{ type: 'desktop-window-update-offered', version: null }]);
	});

	it('tells the asking document when launching the newer window fails or rejects', async () => {
		for (const onJoinable of [
			() => Promise.resolve(false),
			() => Promise.reject(new Error('spawn-enoent')),
		]) {
			const update = new DesktopWindowUpdate({
				desktopRoot: ROOT,
				fallbackFrom: null,
				onJoinable,
				ownVersion: '0.2.0',
				readPointer: () => Promise.resolve('0.3.0'),
				recordEvent: () => undefined,
				startVersionClient: () => Promise.resolve(started(['compatibility-window'])),
				watchPointer: () => () => undefined,
			});
			update.start();
			await settle();
			const document = fakePort();
			update.serve(document.port);
			document.request('0.3.0', PLACE);
			await settle();
			expect(document.sent.at(-1)).toEqual({ type: 'desktop-window-update-failed' });
		}
	});
});
