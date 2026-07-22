import type { TabSlotViewState } from './TabSlotViewState';

export interface TabSlotAnimationHost {
	onTabSlotAnimationProgressed(slot: TabSlotViewState | null): void;
	layoutTabSlotImmediately?(slot: TabSlotViewState): void;
}

/** Ports the lifecycle ordering in Chromium's TabSlotAnimationDelegate. */
export class TabSlotAnimationDelegate {
	private readonly host: TabSlotAnimationHost;
	private slot: TabSlotViewState | null;
	private observing = true;

	public constructor(host: TabSlotAnimationHost, slot: TabSlotViewState) {
		this.host = host;
		this.slot = slot;
		slot.setAnimating(true);
	}

	public animationProgressed(): void {
		this.host.onTabSlotAnimationProgressed(this.slot);
	}

	public animationEnded(): void {
		const current = this.slot;
		current?.setAnimating(false);
		this.animationProgressed();
		if (current) this.host.layoutTabSlotImmediately?.(current);
	}

	public animationCanceled(): void {
		this.animationEnded();
	}

	public onViewDeleting(slot: TabSlotViewState): void {
		if (slot !== this.slot) throw new Error('Observed a different tab slot.');
		this.slot = null;
	}

	public stopObserving(): void {
		this.observing = false;
	}

	public get isObserving(): boolean {
		return this.observing;
	}
}
