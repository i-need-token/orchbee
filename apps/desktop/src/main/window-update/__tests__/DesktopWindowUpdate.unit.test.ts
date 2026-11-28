import {
	DesktopRelaunchPlaceSchema,
	type DesktopRelaunchPlace,
	type DesktopWindowUpdateMainMessage,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import type {
	ServerStartAnswer,
	SessionMoves,
	UpdateRestart,
	UpdateRestartRequest,
} from '../../server-connection';
import {
	DesktopWindowUpdate,
	type DesktopWindowUpdateFocus,
	type DesktopWindowUpdatePort,
} from '../DesktopWindowUpdate';

const ROOT = 'C:\\Programs\\Orchbee\\desktop';
const PLACE = DesktopRelaunchPlaceSchema.parse({
	kind: 'workspace',
	workspaceId: '0123456789abcdef0123456789abcdef',
});

function fakePort() {
	let receive: ((event: { readonly data: unknown }) => void) | undefined;
	let focused = false;
	const focusListeners = new Set<() => void>();
	const sent: DesktopWindowUpdateMainMessage[] = [];
	const port: DesktopWindowUpdatePort = {
		close: vi.fn<() => void>(),
		on: (_event, listener) => {
			receive = listener;
		},
		postMessage: (message) => sent.push(message),
		start: vi.fn<() => void>(),
	};
	const focus: DesktopWindowUpdateFocus = {
		isFocused: () => focused,
		onFocus: (listener) => {
			focusListeners.add(listener);
			return () => focusListeners.delete(listener);
		},
	};
	return {
		focus,
		gainFocus: () => {
			focused = true;
			for (const listener of focusListeners) listener();
		},
		placeReply: (requestId: string, place: DesktopRelaunchPlace | null) =>
			receive?.({ data: { place, requestId, type: 'desktop-window-update-place' } }),
		port,
		refresh: () => receive?.({ data: { type: 'desktop-window-update-refresh' } }),
		request: (version: string, place: DesktopRelaunchPlace | null = null) =>
			receive?.({ data: { place, type: 'desktop-window-update-requested', version } }),
		sent,
	};
}

/** The request id of the place request a document was sent. */
function requestIdOf(sent: readonly DesktopWindowUpdateMainMessage[]): string {
	const request = sent.find((message) => message.type === 'desktop-window-update-place-requested');
	if (request?.type !== 'desktop-window-update-place-requested') {
		throw new Error('No place request was sent.');
	}
	return request.requestId;
}

/** What the new version's Client does where a Session restart is not run: macOS's answer. */
function stopNothing(): Promise<boolean> {
	return Promise.resolve(false);
}

function restartAsBefore(
	_root: string,
	_version: string,
	{ joins }: UpdateRestartRequest,
): Promise<UpdateRestart> {
	return Promise.resolve(joins ? 'not-needed' : 'failed');
}

/** A Session as new as this window, which moves nothing. */
function restartNothing(): Promise<UpdateRestart> {
	return Promise.resolve('not-needed');
}

/** A Session with no terminal on a Terminal Host older than its Worker. */
function noMoves(): Promise<SessionMoves | undefined> {
	return Promise.resolve(null);
}

/** A start answer from a Session that runs `sessionVersion`. */
function runs(sessionVersion: string): ServerStartAnswer {
	return { ...started([]), sessionVersion };
}

function started(negotiatedCapabilities: readonly string[]): ServerStartAnswer {
	return { negotiatedCapabilities, restore: { kind: 'intact' }, sessionVersion: null };
}

function create(options: {
	readonly active: string | null;
	readonly readSessionMoves?: () => Promise<SessionMoves | undefined>;
	readonly restartForVersion?: (
		root: string,
		version: string,
		request: UpdateRestartRequest,
	) => Promise<UpdateRestart>;
	readonly restartSession?: (request: UpdateRestartRequest) => Promise<UpdateRestart>;
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
		readSessionMoves: options.readSessionMoves ?? noMoves,
		recordEvent: () => undefined,
		restartForVersion: options.restartForVersion ?? restartAsBefore,
		restartSession: options.restartSession ?? restartNothing,
		stopSession: stopNothing,
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
		update.serve(document.port, document.focus);
		expect(document.sent).toEqual([
			{ offer: { kind: 'window', version: '0.3.0' }, type: 'desktop-window-update-offered' },
		]);
		movePointer('0.2.0');
		await settle();
		expect(document.sent.at(-1)).toEqual({ offer: null, type: 'desktop-window-update-offered' });
	});

	it('says the update waits when the new version cannot join this Session', async () => {
		const { onJoinable, startVersionClient, update } = create({
			active: '0.3.0',
			startVersionClient: () => Promise.resolve(started(['agent-input-area'])),
		});
		await settle();
		const document = fakePort();
		update.serve(document.port, document.focus);
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
		update.serve(document.port, document.focus);
		document.request('0.3.0', PLACE);
		await settle();
		expect(onJoinable).toHaveBeenCalledWith('0.3.0', PLACE);
		expect(document.sent).not.toContainEqual({ type: 'desktop-window-update-waiting' });
	});

	it('switches once the new version restarted a Session it could not join', async () => {
		const restartForVersion = vi.fn<
			(root: string, version: string, request: UpdateRestartRequest) => Promise<UpdateRestart>
		>(() => Promise.resolve('restarted'));
		const { onJoinable, update } = create({
			active: '0.3.0',
			restartForVersion,
			startVersionClient: () => Promise.resolve(started([])),
		});
		await settle();
		const document = fakePort();
		update.serve(document.port, document.focus);
		document.request('0.3.0', PLACE);
		await settle();
		expect(restartForVersion).toHaveBeenCalledWith(ROOT, '0.3.0', {
			joins: false,
			sessionOlder: false,
		});
		expect(onJoinable).toHaveBeenCalledWith('0.3.0', PLACE);
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
		update.serve(document.port, document.focus);
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
			readSessionMoves: noMoves,
			recordEvent: () => undefined,
			restartForVersion: restartAsBefore,
			restartSession: restartNothing,
			stopSession: stopNothing,
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
		update.serve(document.port, document.focus);
		active = '0.3.0';
		changed?.();
		await settle();
		expect(document.sent.at(-1)).toEqual({
			offer: { kind: 'window', version: '0.3.0' },
			type: 'desktop-window-update-offered',
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
			readSessionMoves: noMoves,
			recordEvent: () => undefined,
			restartForVersion: restartAsBefore,
			restartSession: restartNothing,
			stopSession: stopNothing,
			startVersionClient: () => Promise.resolve(started(['compatibility-window'])),
			watchPointer: () => () => undefined,
		});
		update.start();
		await settle();
		const document = fakePort();
		update.serve(document.port, document.focus);
		document.request('0.3.0');
		await settle();
		expect(readPointer).not.toHaveBeenCalled();
		expect(document.sent).toEqual([{ offer: null, type: 'desktop-window-update-offered' }]);
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
				readSessionMoves: noMoves,
				recordEvent: () => undefined,
				restartForVersion: restartAsBefore,
				restartSession: restartNothing,
				stopSession: stopNothing,
				startVersionClient: () => Promise.resolve(started(['compatibility-window'])),
				watchPointer: () => () => undefined,
			});
			update.start();
			await settle();
			const document = fakePort();
			update.serve(document.port, document.focus);
			document.request('0.3.0', PLACE);
			await settle();
			expect(document.sent.at(-1)).toEqual({ type: 'desktop-window-update-failed' });
		}
	});

	it('refreshes the pointer from disk when a document reports an update check settled', async () => {
		let active: string | null = '0.2.0';
		const update = new DesktopWindowUpdate({
			desktopRoot: ROOT,
			fallbackFrom: null,
			onJoinable: () => Promise.resolve(),
			ownVersion: '0.2.0',
			readPointer: () => Promise.resolve(active),
			readSessionMoves: noMoves,
			recordEvent: () => undefined,
			restartForVersion: restartAsBefore,
			restartSession: restartNothing,
			stopSession: stopNothing,
			startVersionClient: () => Promise.resolve(started(['compatibility-window'])),
			// Simulates OS `fs.watch` coalescing or missing the pointer rename.
			watchPointer: () => () => undefined,
		});
		update.start();
		await settle();
		const document = fakePort();
		update.serve(document.port, document.focus);
		expect(document.sent).toEqual([{ offer: null, type: 'desktop-window-update-offered' }]);

		active = '0.3.0';
		document.refresh();
		await settle();
		expect(document.sent.at(-1)).toEqual({
			offer: { kind: 'window', version: '0.3.0' },
			type: 'desktop-window-update-offered',
		});
	});

	it('switches on switchIfOffered after refreshing from disk, and stays when not joinable or not offered', async () => {
		let active: string | null = '0.2.0';
		let joinable = false;
		let launcherStarts = true;
		const onJoinable = vi.fn<
			(version: string, place: DesktopRelaunchPlace | null) => Promise<boolean>
		>(() => Promise.resolve(launcherStarts));
		const update = new DesktopWindowUpdate({
			desktopRoot: ROOT,
			fallbackFrom: '0.2.5',
			onJoinable,
			ownVersion: '0.2.0',
			readPointer: () => Promise.resolve(active),
			readSessionMoves: noMoves,
			recordEvent: () => undefined,
			restartForVersion: restartAsBefore,
			restartSession: restartNothing,
			stopSession: stopNothing,
			startVersionClient: () =>
				Promise.resolve(started(joinable ? ['compatibility-window'] : ['agent-input-area'])),
			watchPointer: () => () => undefined,
		});
		update.start();
		await settle();

		// Up-to-date pointer: does not switch.
		expect(await update.switchIfOffered()).toBe(false);
		expect(onJoinable).not.toHaveBeenCalled();

		// Fallback version: never offered or switched to.
		active = '0.2.5';
		expect(await update.switchIfOffered()).toBe(false);
		expect(onJoinable).not.toHaveBeenCalled();

		// Newer version on disk before fs.watch fires, but Session lacks compatibility-window: stays.
		active = '0.3.0';
		expect(await update.switchIfOffered()).toBe(false);
		expect(onJoinable).not.toHaveBeenCalled();

		// Joinable, but launcher failed to start: reports false so caller focuses existing window.
		joinable = true;
		launcherStarts = false;
		expect(await update.switchIfOffered(() => PLACE)).toBe(false);
		expect(onJoinable).toHaveBeenCalledWith('0.3.0', PLACE);

		// Joinable and launcher starts: reports true.
		launcherStarts = true;
		expect(await update.switchIfOffered(() => PLACE)).toBe(true);
		expect(onJoinable).toHaveBeenLastCalledWith('0.3.0', PLACE);
	});

	it('asks the focused document for its place and resolves with the place it answers with', async () => {
		const { update } = create({ active: '0.2.0' });
		const elsewhere = fakePort();
		const focused = fakePort();
		update.serve(elsewhere.port, elsewhere.focus);
		update.serve(focused.port, focused.focus);
		focused.gainFocus();
		const asked = update.askPlace();
		focused.placeReply(requestIdOf(focused.sent), PLACE);
		expect(await asked).toEqual(PLACE);
		expect(elsewhere.sent.map((message) => message.type)).not.toContain(
			'desktop-window-update-place-requested',
		);
	});

	it('resolves with no place when the asked document does not reply within a second', async () => {
		vi.useFakeTimers();
		try {
			const { update } = create({ active: '0.2.0' });
			const document = fakePort();
			update.serve(document.port, document.focus);
			document.gainFocus();
			const asked = update.askPlace();
			await vi.advanceTimersByTimeAsync(1_000);
			expect(await asked).toBeNull();
		} finally {
			vi.useRealTimers();
		}
	});

	it('resolves with no place when the reply names another request', async () => {
		const { update } = create({ active: '0.2.0' });
		const document = fakePort();
		update.serve(document.port, document.focus);
		document.gainFocus();
		const asked = update.askPlace();
		document.placeReply('another-request', PLACE);
		expect(await asked).toBeNull();
	});

	it('offers the move once a failed move stopped moving terminals off an older Host, and Update restarts the Session', async () => {
		const restartSession = vi.fn<(request: UpdateRestartRequest) => Promise<UpdateRestart>>(() =>
			Promise.resolve('restarted'),
		);
		const { update } = create({
			active: null,
			readSessionMoves: () => Promise.resolve({ stopped: true, version: '0.1.0' }),
			restartSession,
		});
		const document = fakePort();
		update.serve(document.port, document.focus);
		await update.moveSession(runs('0.2.0'));
		await settle();
		expect(document.sent.at(-1)).toEqual({
			offer: { kind: 'session', sessionVersion: '0.1.0', state: 'moves-stopped', version: '0.2.0' },
			type: 'desktop-window-update-offered',
		});
		// Another document's connect keeps the offer instead of withdrawing and offering it again.
		const sent = document.sent.length;
		await update.moveSession(runs('0.2.0'));
		await settle();
		expect(document.sent).toHaveLength(sent);
		document.request('0.2.0');
		await settle();
		expect(restartSession).toHaveBeenCalledWith({ joins: true, sessionOlder: true });
		expect(document.sent.at(-1)).toEqual({ offer: null, type: 'desktop-window-update-offered' });
	});

	it('withdraws the stopped moves once they run again, and stops reading once none is left', async () => {
		vi.useFakeTimers();
		try {
			const reads: (SessionMoves | undefined)[] = [
				{ stopped: true, version: '0.1.0' },
				{ stopped: false, version: '0.1.0' },
				undefined,
				null,
			];
			let next = 0;
			const readSessionMoves = vi.fn<() => Promise<SessionMoves | undefined>>(() =>
				Promise.resolve(next < reads.length ? reads[next++] : null),
			);
			const { update } = create({ active: null, readSessionMoves });
			const document = fakePort();
			update.serve(document.port, document.focus);
			await update.moveSession(runs('0.2.0'));
			await vi.advanceTimersByTimeAsync(0);
			expect(document.sent.at(-1)).toMatchObject({
				offer: { state: 'moves-stopped' },
				type: 'desktop-window-update-offered',
			});
			await update.moveSession(runs('0.2.0'));
			await vi.advanceTimersByTimeAsync(60_000);
			expect(document.sent.at(-1)).toEqual({ offer: null, type: 'desktop-window-update-offered' });
			expect(readSessionMoves).toHaveBeenCalledTimes(4);
		} finally {
			vi.useRealTimers();
		}
	});
});
