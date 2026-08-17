import {
	createDesktopWindowState,
	resolveDesktopWindowLaunchState,
	type DesktopWindowBounds,
	type DesktopWindowDisplayTopology,
	type DesktopWindowLaunchState,
	type DesktopWindowStateStore,
} from './desktop-window-state';

type DesktopWindowStateEvent =
	| 'close'
	| 'maximize'
	| 'move'
	| 'moved'
	| 'resize'
	| 'resized'
	| 'unmaximize';

export interface DesktopWindowStatePort {
	getNormalBounds(): DesktopWindowBounds;
	isDestroyed(): boolean;
	isFullScreen(): boolean;
	isMaximized(): boolean;
	off(event: DesktopWindowStateEvent, listener: () => void): unknown;
	on(event: DesktopWindowStateEvent, listener: () => void): unknown;
}

export interface DesktopWindowStateAttachment {
	capture(): boolean;
	dispose(): void;
}

export class DesktopWindowStateOwner {
	readonly #platform: NodeJS.Platform;
	readonly #store: DesktopWindowStateStore;
	#writeTail = Promise.resolve();

	constructor(store: DesktopWindowStateStore, platform: NodeJS.Platform) {
		this.#platform = platform;
		this.#store = store;
	}

	attach(window: DesktopWindowStatePort): DesktopWindowStateAttachment {
		let active = true;
		const capture = (): boolean => {
			if (!active || window.isDestroyed() || window.isFullScreen()) return false;
			const state = createDesktopWindowState(window.getNormalBounds(), window.isMaximized());
			if (state === null) return false;
			this.#writeTail = this.#writeTail
				.then(async () => {
					await this.#store.save(state);
				})
				.catch(() => undefined);
			return true;
		};
		const completedGeometryEvents =
			this.#platform === 'linux' ? (['move', 'resize'] as const) : (['moved', 'resized'] as const);
		const events = [
			...completedGeometryEvents,
			'maximize',
			'unmaximize',
			'close',
		] as const satisfies readonly DesktopWindowStateEvent[];
		for (const event of events) window.on(event, capture);

		return {
			capture,
			dispose: () => {
				if (!active) return;
				active = false;
				for (const event of events) window.off(event, capture);
			},
		};
	}

	async flush(): Promise<void> {
		await this.#writeTail;
		await this.#store.flush();
	}

	async loadLaunchState(topology: DesktopWindowDisplayTopology): Promise<DesktopWindowLaunchState> {
		await this.#writeTail;
		const loaded = await this.#store.load().catch(() => ({ kind: 'unavailable' as const }));
		return resolveDesktopWindowLaunchState(
			loaded.kind === 'available' ? loaded.state : null,
			topology,
		);
	}
}
