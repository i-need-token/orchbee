import {
	DesktopWindowLayoutOperationResultSchema,
	DesktopWindowLayoutDestinationSchema,
	DesktopWindowLayoutSnapshotSchema,
	type DesktopWindowLayoutDestination,
	type DesktopWindowLayoutDragSessionId,
	type DesktopWindowLayoutFailureCode,
	type DesktopWindowLayoutOperationResult,
	type DesktopWindowLayoutSnapshot,
} from '@orchbee/desktop-client-protocol';

import type { DesktopWindowBounds } from '../window-state';
import type { DesktopWindowLayoutBrokerController } from './DesktopWindowLayoutBroker';
import type {
	DesktopWindowLayout,
	DesktopWindowLayoutDocument,
	DesktopWindowLayoutWindow,
} from './desktop-window-layout';
import type { DesktopWindowLayoutOwner } from './desktop-window-layout-owner';

export interface DesktopWindowLayoutProductDocument extends DesktopWindowLayoutDocument {
	readonly terminalContexts: readonly {
		readonly tabs: readonly { readonly group: string | null; readonly id: string }[];
		readonly tabIds: readonly string[];
		readonly terminalContextId: string;
	}[];
}

export interface DesktopWindowLayoutHost {
	activateWindow(windowId: string): void;
	discardPreparedWindow(windowId: string): Promise<void>;
	getNewWindowBounds(
		sourceWindowId: string,
		point: Readonly<{ x: number; y: number }> | undefined,
	): DesktopWindowBounds;
	getWindowIdAtPoint(point: Readonly<{ x: number; y: number }>): string | null;
	prepareWindow(window: DesktopWindowLayoutWindow): Promise<boolean>;
	retireWindow(windowId: string): void;
}

export type DesktopWindowLayoutCoordinatorCloseResult =
	| { readonly destinationWindowId: string; readonly kind: 'completed' }
	| { readonly kind: 'last-window' }
	| { readonly kind: 'failed'; readonly reason: 'persistence' | 'stale-window' };

interface DragRecord {
	readonly sourceWindowId: string;
	readonly tabIds: readonly string[];
	readonly terminalContextId: string;
}

export class DesktopWindowLayoutCoordinator {
	readonly #activeTerminalContexts = new Map<string, string>();
	#disposed = false;
	#document: DesktopWindowLayoutProductDocument;
	readonly #drags = new Map<DesktopWindowLayoutDragSessionId, DragRecord>();
	readonly #host: DesktopWindowLayoutHost;
	readonly #listeners = new Set<() => void>();
	readonly #owner: DesktopWindowLayoutOwner;
	#revision = 0n;
	readonly #unsubscribeOwner: () => void;

	public constructor(
		owner: DesktopWindowLayoutOwner,
		document: DesktopWindowLayoutProductDocument,
		host: DesktopWindowLayoutHost,
	) {
		this.#owner = owner;
		this.#document = document;
		this.#host = host;
		this.#unsubscribeOwner = owner.subscribe(() => {
			this.#revision += 1n;
			this.#emit();
		});
	}

	public createBrokerController(windowId: string): DesktopWindowLayoutBrokerController {
		return {
			cancelDrag: (sessionId) => this.#cancelDrag(windowId, sessionId),
			completeDrag: (sessionId, point) => this.#completeDrag(windowId, sessionId, point),
			getSnapshot: () => this.getSnapshot(windowId),
			moveGroup: (terminalContextId, groupId, destination) =>
				this.#moveGroup(windowId, terminalContextId, groupId, destination),
			moveTabs: (terminalContextId, tabIds, destination) =>
				this.#moveTabs(windowId, terminalContextId, tabIds, destination),
			setActiveTerminalContext: (terminalContextId) => {
				this.#activeTerminalContexts.set(windowId, terminalContextId);
			},
			startDrag: (sessionId, terminalContextId, tabIds) => {
				this.#startDrag(windowId, sessionId, terminalContextId, tabIds);
			},
			subscribe: (listener) => this.subscribe(listener),
			updateDrag: (sessionId, point) => this.#updateDrag(windowId, sessionId, point),
		};
	}

	public dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#unsubscribeOwner();
		this.#listeners.clear();
		this.#drags.clear();
	}

	public getSnapshot(windowId: string): DesktopWindowLayoutSnapshot {
		const layout = this.#owner.getSnapshot();
		const window = layout.windows.find((candidate) => candidate.windowId === windowId);
		return DesktopWindowLayoutSnapshotSchema.parse({
			currentWindowId: windowId,
			revision: this.#revision.toString(),
			terminalContexts: window?.assignments ?? [],
			windows: layout.windows.map((candidate) => ({
				ordinal: candidate.ordinal,
				windowId: candidate.windowId,
			})),
		});
	}

	public get layout(): DesktopWindowLayout {
		return this.#owner.getSnapshot();
	}

	public moveTabsToNewWindow(
		sourceWindowId: string,
		terminalContextId: string,
		tabIds: readonly string[],
	): Promise<DesktopWindowLayoutOperationResult> {
		return this.#moveTabs(sourceWindowId, terminalContextId, tabIds, { kind: 'new' });
	}

	public async reconcileDocument(
		document: DesktopWindowLayoutProductDocument,
		fallbackBounds: DesktopWindowBounds,
	): Promise<void> {
		this.#document = document;
		await this.#owner.reconcileDocument(document, fallbackBounds);
	}

	public async requestWindowClose(
		windowId: string,
	): Promise<DesktopWindowLayoutCoordinatorCloseResult> {
		const result = await this.#owner.closeWindow(windowId);
		if (result.kind === 'last-window') return result;
		if (result.kind === 'blocked') return { kind: 'failed', reason: 'stale-window' };
		if (result.kind === 'failed') return result;
		this.#host.retireWindow(windowId);
		this.#host.activateWindow(result.destinationWindowId);
		return { destinationWindowId: result.destinationWindowId, kind: 'completed' };
	}

	public captureGeometry(
		windowId: string,
		bounds: DesktopWindowBounds,
		maximized: boolean,
	): Promise<unknown> {
		return this.#owner.captureGeometry(windowId, bounds, maximized);
	}

	public focusWindow(windowId: string): Promise<unknown> {
		return this.#owner.focusWindow(windowId);
	}

	public subscribe(listener: () => void): () => void {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	async #moveGroup(
		sourceWindowId: string,
		terminalContextId: string,
		groupId: string,
		destination: DesktopWindowLayoutDestination,
	): Promise<DesktopWindowLayoutOperationResult> {
		const context = this.#document.terminalContexts.find(
			(candidate) => candidate.terminalContextId === terminalContextId,
		);
		const tabIds = context?.tabs.filter((tab) => tab.group === groupId).map((tab) => tab.id);
		if (tabIds === undefined || tabIds.length === 0) return failed('stale-group');
		const sourceTabIds = new Set(this.#tabIdsForWindowContext(sourceWindowId, terminalContextId));
		if (!tabIds.some((tabId) => sourceTabIds.has(tabId))) return failed('stale-group');
		return this.#commitMove(sourceWindowId, tabIds, destination);
	}

	async #moveTabs(
		sourceWindowId: string,
		terminalContextId: string,
		tabIds: readonly string[],
		destination: DesktopWindowLayoutDestination,
		point?: Readonly<{ x: number; y: number }> | undefined,
	): Promise<DesktopWindowLayoutOperationResult> {
		const sourceTabIds = new Set(this.#tabIdsForWindowContext(sourceWindowId, terminalContextId));
		if (tabIds.length === 0 || tabIds.some((tabId) => !sourceTabIds.has(tabId))) {
			return failed('stale-tab');
		}
		return this.#commitMove(sourceWindowId, tabIds, destination, point);
	}

	async #commitMove(
		sourceWindowId: string,
		tabIds: readonly string[],
		destination: DesktopWindowLayoutDestination,
		point?: Readonly<{ x: number; y: number }> | undefined,
	): Promise<DesktopWindowLayoutOperationResult> {
		const moveDestination =
			destination.kind === 'existing'
				? destination
				: {
						bounds: this.#host.getNewWindowBounds(sourceWindowId, point),
						kind: 'new' as const,
					};
		let preparedWindowId: string | undefined;
		const result = await this.#owner.moveTabs(tabIds, moveDestination, {
			compensatePreparation: async (candidate) => {
				if (preparedWindowId === candidate.destinationWindowId) {
					await this.#host.discardPreparedWindow(preparedWindowId);
				}
			},
			prepare:
				destination.kind === 'existing'
					? undefined
					: async (candidate) => {
							const window = candidate.layout.windows.find(
								(item) => item.windowId === candidate.destinationWindowId,
							);
							if (window === undefined || !(await this.#host.prepareWindow(window))) return false;
							preparedWindowId = window.windowId;
							return true;
						},
		});
		if (result.kind === 'failed') {
			return failed(result.reason === 'preparation' ? 'native-window-unavailable' : 'persistence');
		}
		if (result.kind === 'blocked') return failed(mapBlockedReason(result.reason));
		if (result.kind === 'unchanged') return { kind: 'unchanged' };
		this.#host.activateWindow(result.destinationWindowId);
		for (const closedWindowId of result.closedWindowIds) this.#host.retireWindow(closedWindowId);
		return completed(result.destinationWindowId);
	}

	#startDrag(
		sourceWindowId: string,
		sessionId: DesktopWindowLayoutDragSessionId,
		terminalContextId: string,
		tabIds: readonly string[],
	): void {
		if (this.#drags.has(sessionId)) return;
		const sourceTabIds = new Set(this.#tabIdsForWindowContext(sourceWindowId, terminalContextId));
		if (tabIds.length === 0 || tabIds.some((tabId) => !sourceTabIds.has(tabId))) return;
		this.#drags.set(sessionId, {
			sourceWindowId,
			tabIds: [...tabIds],
			terminalContextId,
		});
	}

	#updateDrag(
		sourceWindowId: string,
		sessionId: DesktopWindowLayoutDragSessionId,
		point: Readonly<{ x: number; y: number }>,
	): DesktopWindowLayoutDestination | null {
		const drag = this.#drags.get(sessionId);
		if (drag?.sourceWindowId !== sourceWindowId) return null;
		return this.#resolveDragDestination(sourceWindowId, point);
	}

	async #completeDrag(
		sourceWindowId: string,
		sessionId: DesktopWindowLayoutDragSessionId,
		point: Readonly<{ x: number; y: number }>,
	): Promise<DesktopWindowLayoutOperationResult> {
		const drag = this.#drags.get(sessionId);
		this.#drags.delete(sessionId);
		if (drag?.sourceWindowId !== sourceWindowId) return failed('control-unavailable');
		const destination = this.#resolveDragDestination(sourceWindowId, point);
		if (destination === null) return { kind: 'unchanged' };
		return this.#moveTabs(sourceWindowId, drag.terminalContextId, drag.tabIds, destination, point);
	}

	#cancelDrag(
		sourceWindowId: string,
		sessionId: DesktopWindowLayoutDragSessionId,
	): Promise<DesktopWindowLayoutOperationResult> {
		const drag = this.#drags.get(sessionId);
		this.#drags.delete(sessionId);
		return Promise.resolve(
			drag?.sourceWindowId === sourceWindowId
				? { kind: 'unchanged' }
				: failed('control-unavailable'),
		);
	}

	#resolveDragDestination(
		sourceWindowId: string,
		point: Readonly<{ x: number; y: number }>,
	): DesktopWindowLayoutDestination | null {
		const targetWindowId = this.#host.getWindowIdAtPoint(point);
		if (targetWindowId === sourceWindowId) return null;
		return DesktopWindowLayoutDestinationSchema.parse(
			targetWindowId === null ? { kind: 'new' } : { kind: 'existing', windowId: targetWindowId },
		);
	}

	#tabIdsForWindowContext(windowId: string, terminalContextId: string): readonly string[] {
		return (
			this.#owner
				.getSnapshot()
				.windows.find((window) => window.windowId === windowId)
				?.assignments.find((assignment) => assignment.terminalContextId === terminalContextId)
				?.tabIds ?? []
		);
	}

	#emit(): void {
		for (const listener of this.#listeners) listener();
	}
}

function completed(destinationWindowId: string): DesktopWindowLayoutOperationResult {
	return DesktopWindowLayoutOperationResultSchema.parse({ destinationWindowId, kind: 'completed' });
}

function failed(code: DesktopWindowLayoutFailureCode): DesktopWindowLayoutOperationResult {
	return DesktopWindowLayoutOperationResultSchema.parse({ code, kind: 'failed' });
}

function mapBlockedReason(
	reason: 'invalid-bounds' | 'stale-tab' | 'stale-window' | 'window-capacity',
): 'native-window-unavailable' | 'stale-tab' | 'stale-window' | 'window-capacity' {
	return reason === 'invalid-bounds' ? 'native-window-unavailable' : reason;
}
