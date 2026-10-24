export const ExpandOnHoverLockType = {
	ForceCollapse: 'force-collapse',
	KeepCurrentState: 'keep-current-state',
	KeepExpanded: 'keep-expanded',
} as const;

export type ExpandOnHoverLockType =
	(typeof ExpandOnHoverLockType)[keyof typeof ExpandOnHoverLockType];

interface VerticalExpandOnHoverEnvironment {
	shouldDisplayVerticalTabs: boolean;
	collapsed: boolean;
	enabled: boolean;
	frameActive: boolean;
	hovered: boolean;
	focused: boolean;
}

export interface VerticalExpandOnHoverSnapshot {
	revision: number;
	expanded: boolean;
	waitingToExpand: boolean;
	forceCollapseLocks: number;
	keepCurrentStateLocks: number;
	keepExpandedLocks: number;
}

interface VerticalExpandOnHoverHeuristicOptions {
	useVelocityHeuristic: boolean;
	velocityDelayMs: number;
	minimumSamples: number;
	sampleIntervalMs: number;
	velocityThreshold: number;
	distanceFromInsideEdge: number;
	insideEdgeDelayMs: number;
}

type Listener = () => void;

const DEFAULT_ENVIRONMENT: VerticalExpandOnHoverEnvironment = {
	shouldDisplayVerticalTabs: true,
	collapsed: true,
	enabled: true,
	frameActive: true,
	hovered: false,
	focused: false,
};

/** Current Chromium feature defaults from chrome/browser/ui/tabs/features.cc. */
const DEFAULT_VERTICAL_EXPAND_ON_HOVER_HEURISTIC: Readonly<VerticalExpandOnHoverHeuristicOptions> =
	Object.freeze({
		useVelocityHeuristic: true,
		velocityDelayMs: 50,
		minimumSamples: 3,
		sampleIntervalMs: 10,
		velocityThreshold: 0.25,
		distanceFromInsideEdge: 12,
		insideEdgeDelayMs: 200,
	});

interface PointerPosition {
	x: number;
	insideEdgeX: number;
}

/**
 * RAII-style port of VerticalTabStripExpandOnHoverLock. Call dispose() when
 * the bubble, drag, or overlay holding the lock ends.
 */
export class VerticalTabStripExpandOnHoverLock {
	private controller: VerticalTabStripExpandOnHoverController | null;
	private disposed = false;
	public readonly lockType: ExpandOnHoverLockType;

	public constructor(
		controller: VerticalTabStripExpandOnHoverController | null,
		lockType: ExpandOnHoverLockType,
	) {
		this.controller = controller;
		this.lockType = lockType;
		this.controller?.registerLock(this);
	}

	public dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.controller?.unregisterLock(this);
		this.controller = null;
	}

	public clearControllerOnDestruction(): void {
		this.controller = null;
	}
}

/**
 * DOM-independent state controller for Chromium's vertical expand-on-hover
 * lock precedence, fixed-delay path, and current mouse-velocity heuristic.
 */
export class VerticalTabStripExpandOnHoverController {
	private readonly expandDelayMs: number;
	private environment: VerticalExpandOnHoverEnvironment;
	private readonly heuristic: VerticalExpandOnHoverHeuristicOptions;
	private expanded = false;
	private timer: ReturnType<typeof setTimeout> | null = null;
	private heuristicTimer: ReturnType<typeof setInterval> | null = null;
	private heuristicStartPoint: PointerPosition | null = null;
	private heuristicStartTime: number | null = null;
	private heuristicSamples = 0;
	private lastPointerPosition: PointerPosition | null = null;
	private revision = 0;
	private locks = new Set<VerticalTabStripExpandOnHoverLock>();
	private forceCollapseLockCount = 0;
	private keepCurrentStateLockCount = 0;
	private keepExpandedLockCount = 0;
	private listeners = new Set<Listener>();
	private snapshot: VerticalExpandOnHoverSnapshot;

	public constructor(
		expandDelayMs = 350,
		initialEnvironment: Partial<VerticalExpandOnHoverEnvironment> = {},
		heuristic: Partial<VerticalExpandOnHoverHeuristicOptions> = {},
	) {
		this.expandDelayMs = expandDelayMs;
		this.environment = { ...DEFAULT_ENVIRONMENT, ...initialEnvironment };
		this.heuristic = { ...DEFAULT_VERTICAL_EXPAND_ON_HOVER_HEURISTIC, ...heuristic };
		this.snapshot = this.createSnapshot();
		this.updateState();
	}

	public getSnapshot = (): VerticalExpandOnHoverSnapshot => this.snapshot;

	public subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	public setEnvironment(update: Partial<VerticalExpandOnHoverEnvironment>): void {
		const next = { ...this.environment, ...update };
		if (
			next.shouldDisplayVerticalTabs === this.environment.shouldDisplayVerticalTabs &&
			next.collapsed === this.environment.collapsed &&
			next.enabled === this.environment.enabled &&
			next.frameActive === this.environment.frameActive &&
			next.hovered === this.environment.hovered &&
			next.focused === this.environment.focused
		)
			return;
		this.environment = next;
		this.updateState();
	}

	public setHovered(hovered: boolean): void {
		this.setEnvironment({ hovered });
	}

	public setFocused(focused: boolean): void {
		this.setEnvironment({ focused });
	}

	/**
	 * Records the pointer in viewport coordinates. The inside edge is the edge
	 * adjacent to web contents (the right edge for Chromium's left-side strip).
	 */
	public recordPointerPosition(x: number, insideEdgeX: number, timestamp = Date.now()): void {
		if (!Number.isFinite(x) || !Number.isFinite(insideEdgeX)) return;
		this.lastPointerPosition = { x, insideEdgeX };
		if (!this.heuristic.useVelocityHeuristic || !this.shouldEvaluateHover()) return;
		this.calculateMouseVelocity(timestamp);
	}

	public getLock(lockType: ExpandOnHoverLockType): VerticalTabStripExpandOnHoverLock {
		return new VerticalTabStripExpandOnHoverLock(this, lockType);
	}

	public destroy(): void {
		this.clearTimers();
		for (const lock of this.locks) lock.clearControllerOnDestruction();
		this.locks.clear();
		this.listeners.clear();
	}

	public registerLock(lock: VerticalTabStripExpandOnHoverLock): void {
		if (this.locks.has(lock)) return;
		this.locks.add(lock);
		this.adjustLockCount(lock.lockType, 1);
		this.updateState();
	}

	public unregisterLock(lock: VerticalTabStripExpandOnHoverLock): void {
		if (!this.locks.delete(lock)) return;
		this.adjustLockCount(lock.lockType, -1);
		this.updateState();
	}

	private updateState(): void {
		const environment = this.environment;
		if (!environment.shouldDisplayVerticalTabs || !environment.collapsed) {
			this.clearTimers();
			this.setExpanded(false);
			return;
		}

		if (this.forceCollapseLockCount > 0) {
			this.clearTimers();
			this.setExpanded(false);
			return;
		}

		if (this.keepCurrentStateLockCount > 0) {
			this.clearTimers();
			this.commit();
			return;
		}

		if (this.keepExpandedLockCount > 0) {
			this.clearTimers();
			this.setExpanded(true);
			return;
		}

		if (!environment.frameActive) {
			this.clearTimers();
			this.setExpanded(false);
			return;
		}

		const shouldExpand = environment.enabled && (environment.hovered || environment.focused);
		if (!shouldExpand) {
			this.clearTimers();
			this.setExpanded(false);
			return;
		}
		if (this.expanded || this.timer !== null || this.heuristicTimer !== null) return;

		if (
			this.heuristic.useVelocityHeuristic &&
			environment.hovered &&
			this.lastPointerPosition !== null
		) {
			this.startVelocityHeuristic(Date.now());
			return;
		}

		this.timer = setTimeout(() => {
			this.timer = null;
			this.setExpanded(true);
		}, this.expandDelayMs);
		this.commit();
	}

	private adjustLockCount(type: ExpandOnHoverLockType, delta: 1 | -1): void {
		if (type === ExpandOnHoverLockType.ForceCollapse) this.forceCollapseLockCount += delta;
		else if (type === ExpandOnHoverLockType.KeepCurrentState)
			this.keepCurrentStateLockCount += delta;
		else this.keepExpandedLockCount += delta;

		if (
			this.forceCollapseLockCount < 0 ||
			this.keepCurrentStateLockCount < 0 ||
			this.keepExpandedLockCount < 0
		) {
			throw new Error('Expand-on-hover lock count underflow.');
		}
	}

	private shouldEvaluateHover(): boolean {
		return (
			this.environment.shouldDisplayVerticalTabs &&
			this.environment.collapsed &&
			this.environment.enabled &&
			this.environment.frameActive &&
			this.environment.hovered &&
			!this.expanded &&
			this.forceCollapseLockCount === 0 &&
			this.keepCurrentStateLockCount === 0 &&
			this.keepExpandedLockCount === 0
		);
	}

	private startVelocityHeuristic(timestamp: number): void {
		if (this.lastPointerPosition === null || this.heuristicTimer !== null) return;
		this.heuristicStartPoint = { ...this.lastPointerPosition };
		this.heuristicStartTime = timestamp;
		this.heuristicSamples = 1;
		this.heuristicTimer = setInterval(
			() => this.calculateMouseVelocity(Date.now()),
			this.heuristic.sampleIntervalMs,
		);
		this.commit();
	}

	private calculateMouseVelocity(timestamp: number): void {
		if (!this.shouldEvaluateHover() || this.lastPointerPosition === null) {
			this.clearTimers();
			this.commit();
			return;
		}
		if (this.heuristicStartPoint === null || this.heuristicStartTime === null) {
			this.startVelocityHeuristic(timestamp);
			return;
		}

		this.heuristicSamples += 1;
		const elapsed = timestamp - this.heuristicStartTime;
		if (elapsed <= this.heuristic.velocityDelayMs) return;

		const distanceFromInsideEdge = Math.abs(
			this.lastPointerPosition.x - this.lastPointerPosition.insideEdgeX,
		);
		if (
			elapsed <= this.heuristic.insideEdgeDelayMs &&
			distanceFromInsideEdge <= this.heuristic.distanceFromInsideEdge
		)
			return;

		const distance = Math.abs(this.lastPointerPosition.x - this.heuristicStartPoint.x);
		if (
			this.heuristicSamples >= this.heuristic.minimumSamples &&
			distance / elapsed < this.heuristic.velocityThreshold
		) {
			this.clearTimers();
			this.setExpanded(true);
		}
	}

	private clearTimers(): void {
		if (this.timer !== null) clearTimeout(this.timer);
		if (this.heuristicTimer !== null) clearInterval(this.heuristicTimer);
		this.timer = null;
		this.heuristicTimer = null;
		this.heuristicStartPoint = null;
		this.heuristicStartTime = null;
		this.heuristicSamples = 0;
	}

	private setExpanded(expanded: boolean): void {
		if (this.expanded === expanded) {
			this.commit();
			return;
		}
		this.expanded = expanded;
		this.commit();
	}

	private commit(): void {
		const next = this.createSnapshot();
		if (
			next.expanded === this.snapshot.expanded &&
			next.waitingToExpand === this.snapshot.waitingToExpand &&
			next.forceCollapseLocks === this.snapshot.forceCollapseLocks &&
			next.keepCurrentStateLocks === this.snapshot.keepCurrentStateLocks &&
			next.keepExpandedLocks === this.snapshot.keepExpandedLocks
		)
			return;
		this.revision += 1;
		this.snapshot = { ...next, revision: this.revision };
		this.listeners.forEach((listener) => listener());
	}

	private createSnapshot(): VerticalExpandOnHoverSnapshot {
		return Object.freeze({
			revision: this.revision,
			expanded: this.expanded,
			waitingToExpand: this.timer !== null || this.heuristicTimer !== null,
			forceCollapseLocks: this.forceCollapseLockCount,
			keepCurrentStateLocks: this.keepCurrentStateLockCount,
			keepExpandedLocks: this.keepExpandedLockCount,
		});
	}
}
