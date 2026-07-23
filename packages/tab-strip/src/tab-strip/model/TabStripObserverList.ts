export interface TabStripObserver {
	onTabAdded?(index: number): void;
	onTabMoved?(fromIndex: number, toIndex: number): void;
	onTabRemoved?(index: number): void;
}

/** Small observer-list equivalent for platform-independent tab strip hosts. */
export class TabStripObserverList {
	private readonly observers = new Set<TabStripObserver>();

	public add(observer: TabStripObserver): () => void {
		this.observers.add(observer);
		return () => this.observers.delete(observer);
	}

	public notifyTabAdded(index: number): void {
		this.observers.forEach((observer) => observer.onTabAdded?.(index));
	}

	public notifyTabMoved(fromIndex: number, toIndex: number): void {
		this.observers.forEach((observer) => observer.onTabMoved?.(fromIndex, toIndex));
	}

	public notifyTabRemoved(index: number): void {
		this.observers.forEach((observer) => observer.onTabRemoved?.(index));
	}
}
