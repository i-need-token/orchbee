import type { DesktopWindowBounds } from '../window-state';
import {
	closeDesktopWindow,
	focusDesktopWindow,
	inspectDesktopWindowLayoutInvariantViolations,
	moveDesktopWindowTabs,
	normalizeDesktopWindowLayout,
	updateDesktopWindowGeometry,
	type DesktopWindowLayout,
	type DesktopWindowLayoutCloseResult,
	type DesktopWindowLayoutDocument,
	type DesktopWindowLayoutFocusResult,
	type DesktopWindowLayoutGeometryResult,
	type DesktopWindowLayoutMoveDestination,
	type DesktopWindowLayoutMoveResult,
	type DesktopWindowLayoutStore,
} from './desktop-window-layout';

export type DesktopWindowLayoutOwnerMoveResult =
	| DesktopWindowLayoutMoveResult
	| { readonly kind: 'failed'; readonly reason: 'persistence' | 'preparation' };

export interface DesktopWindowLayoutOwnerMoveOptions {
	readonly compensatePreparation?:
		| ((
				result: Extract<DesktopWindowLayoutMoveResult, { readonly kind: 'completed' }>,
		  ) => Promise<void>)
		| undefined;
	readonly prepare?:
		| ((
				result: Extract<DesktopWindowLayoutMoveResult, { readonly kind: 'completed' }>,
		  ) => Promise<boolean>)
		| undefined;
}

export type DesktopWindowLayoutOwnerCloseResult =
	| DesktopWindowLayoutCloseResult
	| { readonly kind: 'failed'; readonly reason: 'persistence' };

export type DesktopWindowLayoutOwnerLocalResult<
	Result extends DesktopWindowLayoutFocusResult | DesktopWindowLayoutGeometryResult,
> = Result extends { readonly kind: 'completed'; readonly layout: DesktopWindowLayout }
	? Result & { readonly persisted: boolean }
	: Result;

export interface DesktopWindowLayoutOwnerReconcileResult {
	readonly changed: boolean;
	readonly kind: 'completed';
	readonly persisted: boolean;
	readonly recovered: boolean;
}

export class DesktopWindowLayoutOwner {
	#disposed = false;
	#document: DesktopWindowLayoutDocument;
	#layout: DesktopWindowLayout;
	readonly #listeners = new Set<() => void>();
	#operationTail = Promise.resolve();
	readonly #store: DesktopWindowLayoutStore;

	constructor(
		store: DesktopWindowLayoutStore,
		document: DesktopWindowLayoutDocument,
		layout: DesktopWindowLayout,
	) {
		this.#store = store;
		this.#document = document;
		this.#layout = layout;
	}

	dispose(): void {
		this.#disposed = true;
		this.#listeners.clear();
	}

	async flush(): Promise<void> {
		await this.#operationTail;
		await this.#store.flush();
	}

	getSnapshot(): DesktopWindowLayout {
		return this.#layout;
	}

	moveTabs(
		tabIds: readonly string[],
		destination: DesktopWindowLayoutMoveDestination,
		options: DesktopWindowLayoutOwnerMoveOptions = {},
	): Promise<DesktopWindowLayoutOwnerMoveResult> {
		return this.#enqueue(async () => {
			const result = moveDesktopWindowTabs(this.#layout, this.#document, tabIds, destination);
			if (result.kind !== 'completed') return result;
			if (options.prepare !== undefined && !(await options.prepare(result))) {
				return { kind: 'failed', reason: 'preparation' };
			}
			if (!(await this.#save(result.layout))) {
				await options.compensatePreparation?.(result).catch(() => undefined);
				return { kind: 'failed', reason: 'persistence' };
			}
			this.#publish(result.layout);
			return result;
		});
	}

	closeWindow(windowId: string): Promise<DesktopWindowLayoutOwnerCloseResult> {
		return this.#enqueue(async () => {
			const result = closeDesktopWindow(this.#layout, this.#document, windowId);
			if (result.kind !== 'completed') return result;
			if (!(await this.#save(result.layout))) return { kind: 'failed', reason: 'persistence' };
			this.#publish(result.layout);
			return result;
		});
	}

	focusWindow(
		windowId: string,
	): Promise<DesktopWindowLayoutOwnerLocalResult<DesktopWindowLayoutFocusResult>> {
		return this.#enqueue(async () => {
			const result = focusDesktopWindow(this.#layout, windowId);
			if (result.kind !== 'completed') return result;
			this.#publish(result.layout);
			return { ...result, persisted: await this.#save(result.layout) };
		});
	}

	captureGeometry(
		windowId: string,
		bounds: DesktopWindowBounds,
		maximized: boolean,
	): Promise<DesktopWindowLayoutOwnerLocalResult<DesktopWindowLayoutGeometryResult>> {
		return this.#enqueue(async () => {
			const result = updateDesktopWindowGeometry(this.#layout, windowId, bounds, maximized);
			if (result.kind !== 'completed') return result;
			this.#publish(result.layout);
			return { ...result, persisted: await this.#save(result.layout) };
		});
	}

	reconcileDocument(
		document: DesktopWindowLayoutDocument,
		fallbackBounds: DesktopWindowBounds,
	): Promise<DesktopWindowLayoutOwnerReconcileResult> {
		return this.#enqueue(async () => {
			this.#document = document;
			const normalized = normalizeDesktopWindowLayout(this.#layout, document, fallbackBounds);
			if (normalized.changed) this.#publish(normalized.layout);
			return {
				changed: normalized.changed,
				kind: 'completed',
				persisted: !normalized.changed || (await this.#save(normalized.layout)),
				recovered: normalized.kind === 'recovered',
			};
		});
	}

	subscribe(listener: () => void): () => void {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	#enqueue<Result>(operation: () => Promise<Result>): Promise<Result> {
		const result = this.#operationTail.then(operation, operation);
		this.#operationTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	#publish(layout: DesktopWindowLayout): void {
		if (this.#disposed) return;
		this.#layout = layout;
		for (const listener of this.#listeners) {
			try {
				listener();
			} catch {
				// A projection observer cannot invalidate the authoritative layout commit.
			}
		}
	}

	async #save(layout: DesktopWindowLayout): Promise<boolean> {
		if (this.#disposed) return false;
		if (inspectDesktopWindowLayoutInvariantViolations(layout, this.#document).length > 0) {
			return false;
		}
		return this.#store.save(layout).catch(() => false);
	}
}

export async function loadDesktopWindowLayoutOwner(
	store: DesktopWindowLayoutStore,
	document: DesktopWindowLayoutDocument,
	fallbackBounds: DesktopWindowBounds,
	fallbackMaximized = false,
): Promise<DesktopWindowLayoutOwner> {
	const loaded = await store.load().catch(() => ({ kind: 'unavailable' as const }));
	const normalized = normalizeDesktopWindowLayout(
		loaded.kind === 'available' ? loaded.layout : null,
		document,
		fallbackBounds,
		fallbackMaximized,
	);
	const owner = new DesktopWindowLayoutOwner(store, document, normalized.layout);
	if (normalized.changed) await store.save(normalized.layout).catch(() => false);
	return owner;
}
