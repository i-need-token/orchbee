class TestResizeObserverEntry implements ResizeObserverEntry {
	public readonly borderBoxSize: readonly ResizeObserverSize[] = [];
	public readonly contentBoxSize: readonly ResizeObserverSize[] = [];
	public readonly contentRect: DOMRectReadOnly;
	public readonly devicePixelContentBoxSize: readonly ResizeObserverSize[] = [];
	public readonly target: Element;

	public constructor(target: Element) {
		this.target = target;
		this.contentRect = target.getBoundingClientRect();
	}
}

class TestResizeObserver implements ResizeObserver {
	private readonly callback: ResizeObserverCallback;

	public constructor(callback: ResizeObserverCallback) {
		this.callback = callback;
	}

	public observe(target: Element): void {
		this.callback([new TestResizeObserverEntry(target)], this);
	}

	public unobserve(): void {}
	public disconnect(): void {}
}

globalThis.ResizeObserver = TestResizeObserver;
