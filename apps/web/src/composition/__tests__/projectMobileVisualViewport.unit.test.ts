// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';

import { projectMobileVisualViewport } from '../projectMobileVisualViewport';

describe('projectMobileVisualViewport', () => {
	it('projects each valid coarse-pointer viewport sample and restores the root on disposal', () => {
		const fixture = createFixture();
		const root = document.createElement('div');
		root.style.position = 'relative';
		root.style.width = '50%';
		const projection = projectMobileVisualViewport(fixture.targetWindow, root);

		expect(fixture.queriedMedia).toEqual(['(hover: none) and (pointer: coarse)']);
		expect(root.style.position).toBe('fixed');
		expect(root.style.left).toBe('4px');
		expect(root.style.top).toBe('12px');
		expect(root.style.width).toBe('390px');
		expect(root.style.height).toBe('500px');

		fixture.viewport.height = 760;
		fixture.viewport.offsetLeft = 0;
		fixture.viewport.offsetTop = 0;
		fixture.dispatchViewport('resize');
		expect(root.style.height).toBe('760px');
		expect(root.style.top).toBe('0px');

		projection.dispose();
		expect(root.style.position).toBe('relative');
		expect(root.style.left).toBe('');
		expect(root.style.top).toBe('');
		expect(root.style.width).toBe('50%');
		expect(root.style.height).toBe('');
		expect(fixture.listenerCount()).toBe(0);
	});

	it('falls back to original layout geometry for invalid samples and fine pointers', () => {
		const fixture = createFixture();
		const root = document.createElement('div');
		const projection = projectMobileVisualViewport(fixture.targetWindow, root);

		fixture.viewport.width = Number.NaN;
		fixture.dispatchViewport('scroll');
		expect(root.style.position).toBe('');
		expect(root.style.width).toBe('');

		fixture.viewport.width = 390;
		fixture.media.matches = false;
		fixture.dispatchMedia();
		expect(root.style.position).toBe('');

		fixture.media.matches = true;
		fixture.dispatchMedia();
		expect(root.style.position).toBe('fixed');
		projection.dispose();
	});

	it('leaves layout untouched when the VisualViewport API is unavailable', () => {
		const root = document.createElement('div');
		root.style.height = '100%';
		const projection = projectMobileVisualViewport(
			{
				matchMedia: () => {
					throw new Error('matchMedia must not be read without VisualViewport');
				},
				visualViewport: null,
			},
			root,
		);

		expect(root.style.height).toBe('100%');
		projection.dispose();
	});
});

function createFixture() {
	const viewportListeners = new Map<string, Set<EventListener>>();
	const mediaListeners = new Set<EventListener>();
	const queriedMedia: string[] = [];
	const viewport = {
		height: 500,
		offsetLeft: 4,
		offsetTop: 12,
		width: 390,
		addEventListener: (type: 'resize' | 'scroll', listener: EventListener) => {
			const listeners = viewportListeners.get(type) ?? new Set<EventListener>();
			listeners.add(listener);
			viewportListeners.set(type, listeners);
		},
		removeEventListener: (type: 'resize' | 'scroll', listener: EventListener) => {
			viewportListeners.get(type)?.delete(listener);
		},
	};
	const media = {
		matches: true,
		addEventListener: (_type: 'change', listener: EventListener) => mediaListeners.add(listener),
		removeEventListener: (_type: 'change', listener: EventListener) =>
			mediaListeners.delete(listener),
	};
	return {
		dispatchMedia: () => {
			for (const listener of mediaListeners) listener(new Event('change'));
		},
		dispatchViewport: (type: 'resize' | 'scroll') => {
			for (const listener of viewportListeners.get(type) ?? []) listener(new Event(type));
		},
		listenerCount: () =>
			[...viewportListeners.values()].reduce((count, listeners) => count + listeners.size, 0) +
			mediaListeners.size,
		media,
		queriedMedia,
		targetWindow: {
			matchMedia: (query: string) => {
				queriedMedia.push(query);
				return media;
			},
			visualViewport: viewport,
		},
		viewport,
	};
}
