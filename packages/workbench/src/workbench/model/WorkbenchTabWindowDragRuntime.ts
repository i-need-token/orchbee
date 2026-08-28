import {
	CrossWindowDragState,
	type CrossWindowDragEndReason,
	type CrossWindowDragSnapshot,
	type DragPoint,
	type DragSessionData,
	type TabDragRestoreState,
	type TabModel,
	type TabStripCrossWindowDragRuntime,
} from '@orchbee/tab-strip';

import type { WorkbenchTabWindowControl } from '../types/workbench-tab-window-control';

type Listener = () => void;
type WorkbenchTabWindowDragSnapshot = Pick<
	CrossWindowDragSnapshot,
	'attachedContextId' | 'endReason' | 'sourceContextId' | 'state' | 'targetActive'
>;

export interface WorkbenchTabWindowDragRuntimeOptions {
	readonly control: WorkbenchTabWindowControl;
	readonly model: TabModel;
	readonly sessionData: DragSessionData;
	readonly terminalContextId: string;
}

/** Adapts native window hit testing to the TabStrip pointer lifecycle. */
export class WorkbenchTabWindowDragRuntime implements TabStripCrossWindowDragRuntime {
	readonly #control: WorkbenchTabWindowControl;
	#controlUnsubscribe: (() => void) | undefined;
	#endReason: CrossWindowDragEndReason | null = null;
	readonly #listeners = new Set<Listener>();
	#lastPoint: Readonly<DragPoint> | undefined;
	readonly #model: TabModel;
	readonly #restoreState: TabDragRestoreState;
	#sessionId: string | undefined;
	readonly #sourceContextId: string;
	#state: CrossWindowDragState = CrossWindowDragState.NotStarted;
	readonly #tabIds: readonly string[];
	readonly #terminalContextId: string;
	#targetActive = false;
	#targetContextId: string | null = null;

	public constructor(options: WorkbenchTabWindowDragRuntimeOptions) {
		this.#control = options.control;
		this.#model = options.model;
		this.#restoreState = options.model.captureDragRestoreState();
		this.#sourceContextId = options.control.getSnapshot().currentWindowId;
		this.#tabIds = Object.freeze([...options.sessionData.attachedViewIds]);
		this.#terminalContextId = options.terminalContextId;
	}

	public readonly getSnapshot = (): WorkbenchTabWindowDragSnapshot => ({
		attachedContextId: this.#targetContextId,
		endReason: this.#endReason,
		sourceContextId: this.#sourceContextId,
		state: this.#state,
		targetActive: this.#targetActive,
	});

	public readonly subscribe = (listener: Listener): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	public start(): void {
		if (this.#state !== CrossWindowDragState.NotStarted || this.#tabIds.length === 0) return;
		const sessionId = this.#control.startDrag(this.#terminalContextId, this.#tabIds);
		if (sessionId === null) return;
		this.#sessionId = sessionId;
		this.#state = CrossWindowDragState.DraggingTabs;
		this.#controlUnsubscribe = this.#control.subscribe(this.#refreshTarget);
		this.#refreshTarget();
		this.#emit();
	}

	public update(pointInScreen: Readonly<DragPoint>): void {
		if (this.#state !== CrossWindowDragState.DraggingTabs || this.#sessionId === undefined) return;
		this.#lastPoint = pointInScreen;
		this.#control.updateDrag(this.#sessionId, pointInScreen);
	}

	public end(reason: CrossWindowDragEndReason): void {
		if (this.#state === CrossWindowDragState.Stopped) return;
		const sessionId = this.#sessionId;
		const completionPoint =
			reason === 'complete' && this.#targetActive ? this.#lastPoint : undefined;
		const commitsExternalMove = completionPoint !== undefined;
		if (commitsExternalMove) this.#model.restoreCanceledDrag(this.#restoreState);
		this.#state = CrossWindowDragState.Stopped;
		this.#endReason = commitsExternalMove ? 'complete' : reason;
		this.#targetActive = false;
		this.#targetContextId = null;
		this.#controlUnsubscribe?.();
		this.#controlUnsubscribe = undefined;
		this.#emit();
		if (sessionId === undefined) return;
		void this.#control.finishDrag(
			sessionId,
			commitsExternalMove ? { point: completionPoint, reason: 'complete' } : { reason: 'cancel' },
		);
	}

	readonly #refreshTarget = (): void => {
		const sessionId = this.#sessionId;
		if (sessionId === undefined || this.#state !== CrossWindowDragState.DraggingTabs) return;
		const target = this.#control.getDragTarget(sessionId);
		const targetActive = target !== null;
		let targetContextId: string | null = null;
		if (target?.kind === 'existing') targetContextId = target.windowId;
		else if (target?.kind === 'new') targetContextId = `${this.#sourceContextId}:new-window`;
		if (targetActive === this.#targetActive && targetContextId === this.#targetContextId) return;
		this.#targetActive = targetActive;
		this.#targetContextId = targetContextId;
		this.#emit();
	};

	#emit(): void {
		this.#listeners.forEach((listener) => listener());
	}
}
