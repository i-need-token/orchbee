// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { useWorkbenchSidebarDropdownMenuPlacement } from '../useWorkbenchSidebarDropdownMenuPlacement';

class TestResizeObserver implements ResizeObserver {
	public static instances: TestResizeObserver[] = [];

	public readonly callback: ResizeObserverCallback;
	public readonly disconnect = vi.fn<() => void>();
	public readonly observe = vi.fn<(target: Element) => void>();
	public readonly unobserve = vi.fn<(target: Element) => void>();

	public constructor(callback: ResizeObserverCallback) {
		this.callback = callback;
		TestResizeObserver.instances.push(this);
	}
}

afterEach(() => {
	cleanup();
	document.body.replaceChildren();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
	TestResizeObserver.instances = [];
	document.documentElement.style.removeProperty('--ob-ref-size-menu');
	document.documentElement.style.removeProperty('font-size');
});

describe('useWorkbenchSidebarDropdownMenuPlacement', () => {
	it('selects the lower-left fallback only when the right side cannot fit', () => {
		const { result } = renderHook(() => useWorkbenchSidebarDropdownMenuPlacement());
		const trigger = createTrigger(320);
		result.current.triggerRef.current = trigger;
		stubMenuGeometry(578);

		act(() => result.current.onOpenChange(true));

		expect(result.current).toMatchObject({ align: 'end', side: 'bottom' });
	});

	it('re-evaluates right-side placement while the open sidebar changes size', () => {
		vi.stubGlobal('ResizeObserver', TestResizeObserver);
		const { result } = renderHook(() => useWorkbenchSidebarDropdownMenuPlacement());
		const sidebar = document.createElement('aside');
		sidebar.dataset['slot'] = 'workbench-sidebar';
		const trigger = createTrigger(320);
		sidebar.append(trigger);
		document.body.append(sidebar);
		result.current.triggerRef.current = trigger;
		const viewportWidth = vi
			.spyOn(document.documentElement, 'clientWidth', 'get')
			.mockReturnValue(578);
		stubMenuToken();

		act(() => result.current.onOpenChange(true));
		expect(result.current).toMatchObject({ align: 'end', side: 'bottom' });
		const resizeObserver = TestResizeObserver.instances[0];
		expect(resizeObserver?.observe).toHaveBeenCalledWith(document.documentElement);
		expect(resizeObserver?.observe).toHaveBeenCalledWith(sidebar);
		if (resizeObserver === undefined) {
			throw new Error('Expected the menu placement hook to install a ResizeObserver.');
		}

		viewportWidth.mockReturnValue(900);
		act(() => resizeObserver.callback([], resizeObserver));
		expect(result.current).toMatchObject({ align: 'start', side: 'right' });

		act(() => result.current.onOpenChange(false));
		expect(resizeObserver?.disconnect).toHaveBeenCalledOnce();
	});
});

function createTrigger(inlineEnd: number): HTMLButtonElement {
	const trigger = document.createElement('button');
	vi.spyOn(trigger, 'getBoundingClientRect').mockReturnValue({
		bottom: 40,
		height: 24,
		left: inlineEnd - 24,
		right: inlineEnd,
		toJSON: vi.fn<() => unknown>(),
		top: 16,
		width: 24,
		x: inlineEnd - 24,
		y: 16,
	});
	return trigger;
}

function stubMenuGeometry(viewportInlineSize: number): void {
	vi.spyOn(document.documentElement, 'clientWidth', 'get').mockReturnValue(viewportInlineSize);
	stubMenuToken();
}

function stubMenuToken(): void {
	document.documentElement.style.setProperty('--ob-ref-size-menu', '15.625rem');
	document.documentElement.style.fontSize = '16px';
}
