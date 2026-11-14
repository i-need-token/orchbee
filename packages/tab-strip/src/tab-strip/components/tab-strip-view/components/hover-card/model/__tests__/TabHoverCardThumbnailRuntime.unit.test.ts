import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ThumbnailCaptureReadiness, ThumbnailWaitState } from '../tab-hover-card-controller';
import {
	TabHoverCardThumbnailObserver,
	type HoverCardThumbnailSource,
	type HoverCardThumbnailSubscription,
} from '../TabHoverCardThumbnailObserver';
import { TabHoverCardThumbnailRuntime } from '../TabHoverCardThumbnailRuntime';

class FakeThumbnailSource implements HoverCardThumbnailSource<string> {
	public callback: ((image: string) => void) | null = null;
	public disposed = 0;
	public requested = 0;
	public sizeHint: { width: number; height: number } | null = null;
	private readonly data: boolean;
	private readonly readiness: ThumbnailCaptureReadiness;

	public constructor(
		data: boolean,
		readiness: ThumbnailCaptureReadiness = ThumbnailCaptureReadiness.ReadyForInitialCapture,
	) {
		this.data = data;
		this.readiness = readiness;
	}

	public hasData() {
		return this.data;
	}
	public getCaptureReadiness() {
		return this.readiness;
	}
	public requestThumbnailImage() {
		this.requested += 1;
	}
	public subscribe(): HoverCardThumbnailSubscription<string> {
		return {
			setSizeHint: (size) => {
				this.sizeHint = { ...size };
			},
			setImageCallback: (callback) => {
				this.callback = callback;
			},
			dispose: () => {
				this.disposed += 1;
			},
		};
	}
	public emit(image: string) {
		this.callback?.(image);
	}
}

describe('TabHoverCardThumbnailObserver', () => {
	it('sets the preview hint, requests capture, and rejects stale-source callbacks', () => {
		const a = new FakeThumbnailSource(false);
		const b = new FakeThumbnailSource(false);
		const observer = new TabHoverCardThumbnailObserver<string>();
		const received: string[] = [];
		observer.addCallback((_observer, image) => received.push(image));

		observer.observe(a);
		expect(a.sizeHint).toEqual({ width: 320, height: 180 });
		expect(a.requested).toBe(1);
		observer.observe(b);
		expect(a.disposed).toBe(1);
		a.emit('stale');
		b.emit('current');
		expect(received).toEqual(['current']);
	});
});

describe('TabHoverCardThumbnailRuntime', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	const target = (id: string, source: FakeThumbnailSource, update = {}) => ({
		id,
		source,
		previewsEnabled: true,
		needsThumbnail: true,
		isInitialShow: false,
		...update,
	});

	it('delays capture by readiness, shows a placeholder, then publishes the image', () => {
		const source = new FakeThumbnailSource(false, ThumbnailCaptureReadiness.NotReady);
		const runtime = new TabHoverCardThumbnailRuntime<string>();
		runtime.updateTarget(target('tab-a', source));
		expect(runtime.getSnapshot()).toMatchObject({
			targetId: 'tab-a',
			waitState: ThumbnailWaitState.WaitingWithPlaceholder,
			waitingForCapture: true,
			current: { presentation: 'placeholder' },
		});
		vi.advanceTimersByTime(799);
		expect(source.requested).toBe(0);
		vi.advanceTimersByTime(1);
		expect(source.requested).toBe(1);
		source.emit('preview-a');
		expect(runtime.getSnapshot()).toMatchObject({
			waitState: ThumbnailWaitState.NotWaiting,
			current: { presentation: 'thumbnail', image: 'preview-a' },
		});
	});

	it('does not capture missing discarded previews', () => {
		const source = new FakeThumbnailSource(false);
		const runtime = new TabHoverCardThumbnailRuntime<string>();
		runtime.updateTarget(target('discarded', source, { discarded: true }));
		expect(runtime.getSnapshot().current.presentation).toBe('none');
		expect(source.requested).toBe(0);
	});

	it('keeps the old image until the slide crossfade point and cancels on hide', () => {
		const a = new FakeThumbnailSource(true);
		const b = new FakeThumbnailSource(true);
		const runtime = new TabHoverCardThumbnailRuntime<string>();
		runtime.updateTarget(target('a', a, { isInitialShow: true }));
		a.emit('preview-a');
		runtime.updateTarget(target('b', b, { crossfadeStart: 0.25 }));
		expect(runtime.getSnapshot().current.image).toBe('preview-a');
		const placeholderDelay = 200 * 0.25;
		vi.advanceTimersByTime(placeholderDelay - 1);
		expect(runtime.getSnapshot().current.image).toBe('preview-a');
		vi.advanceTimersByTime(1);
		expect(runtime.getSnapshot()).toMatchObject({
			waitState: ThumbnailWaitState.WaitingWithPlaceholder,
			current: { presentation: 'placeholder' },
		});
		runtime.updateTarget(null);
		b.emit('stale-b');
		expect(runtime.getSnapshot()).toMatchObject({
			targetId: null,
			current: { presentation: 'none' },
		});
	});
});
