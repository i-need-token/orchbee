const MOBILE_TERMINAL_POINTER_MEDIA_QUERY = '(hover: none) and (pointer: coarse)';

interface MobileVisualViewport {
	readonly height: number;
	readonly offsetLeft: number;
	readonly offsetTop: number;
	readonly width: number;
	addEventListener(type: 'resize' | 'scroll', listener: EventListener): void;
	removeEventListener(type: 'resize' | 'scroll', listener: EventListener): void;
}

interface MobilePointerMediaQuery {
	readonly matches: boolean;
	addEventListener(type: 'change', listener: EventListener): void;
	removeEventListener(type: 'change', listener: EventListener): void;
}

interface MobileVisualViewportWindow {
	readonly visualViewport: MobileVisualViewport | null;
	matchMedia(query: string): MobilePointerMediaQuery;
}

export interface MobileVisualViewportProjection {
	dispose(): void;
}

interface RootGeometry {
	readonly height: string;
	readonly left: string;
	readonly position: string;
	readonly top: string;
	readonly width: string;
}

/** Projects the browser's visual viewport onto the app root before descendants measure it. */
export function projectMobileVisualViewport(
	targetWindow: MobileVisualViewportWindow,
	rootElement: HTMLElement | null,
): MobileVisualViewportProjection {
	if (rootElement === null) return { dispose: () => undefined };
	const viewport = targetWindow.visualViewport;
	if (viewport === null) return { dispose: () => undefined };
	const media = targetWindow.matchMedia(MOBILE_TERMINAL_POINTER_MEDIA_QUERY);
	const initial: RootGeometry = {
		height: rootElement.style.height,
		left: rootElement.style.left,
		position: rootElement.style.position,
		top: rootElement.style.top,
		width: rootElement.style.width,
	};
	let disposed = false;

	const restore = (): void => {
		rootElement.style.position = initial.position;
		rootElement.style.left = initial.left;
		rootElement.style.top = initial.top;
		rootElement.style.width = initial.width;
		rootElement.style.height = initial.height;
	};
	const apply = (): void => {
		if (disposed || !media.matches) {
			restore();
			return;
		}
		const { height, offsetLeft, offsetTop, width } = viewport;
		if (
			!Number.isFinite(width) ||
			!Number.isFinite(height) ||
			!Number.isFinite(offsetLeft) ||
			!Number.isFinite(offsetTop) ||
			width <= 0 ||
			height <= 0
		) {
			restore();
			return;
		}
		rootElement.style.position = 'fixed';
		rootElement.style.left = `${offsetLeft}px`;
		rootElement.style.top = `${offsetTop}px`;
		rootElement.style.width = `${width}px`;
		rootElement.style.height = `${height}px`;
	};
	const handleViewportChange: EventListener = () => apply();
	const handleMediaChange: EventListener = () => apply();
	viewport.addEventListener('resize', handleViewportChange);
	viewport.addEventListener('scroll', handleViewportChange);
	media.addEventListener('change', handleMediaChange);
	apply();

	return {
		dispose: () => {
			if (disposed) return;
			disposed = true;
			viewport.removeEventListener('resize', handleViewportChange);
			viewport.removeEventListener('scroll', handleViewportChange);
			media.removeEventListener('change', handleMediaChange);
			restore();
		},
	};
}
