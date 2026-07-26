import { requireDefined } from '../../utils/requireDefined';
import { describe, expect, it, vi } from 'vitest';
import type { ProposedLayout } from '../proposed-layout';
import {
	TabCollectionAnimatingLayoutManager,
	TabCollectionAnimationAxis,
	TabCollectionAnimationDirection,
	tabCollectionEaseInOut,
	tabCollectionEaseOut,
} from '../tab-collection-animating-layout-manager';
function stacked(ids: readonly string[], width = 100): ProposedLayout {
	return {
		hostSize: {
			width,
			height: ids.length * 20,
		},
		children: ids.map((id, index) => ({
			id,
			visible: true,
			bounds: {
				x: 0,
				y: index * 20,
				width,
				height: 20,
			},
		})),
	};
}
describe('TabCollectionAnimatingLayoutManager', () => {
	it('uses Chromium BoundsAnimator ease-out progress', () => {
		expect(tabCollectionEaseOut(0)).toBe(0);
		expect(tabCollectionEaseOut(0.5)).toBe(0.75);
		expect(tabCollectionEaseOut(1)).toBe(1);
	});

	it('uses Chromium collection ease-in-out progress', () => {
		expect(tabCollectionEaseInOut(0)).toBe(0);
		expect(tabCollectionEaseInOut(0.25)).toBe(0.125);
		expect(tabCollectionEaseInOut(0.5)).toBe(0.5);
		expect(tabCollectionEaseInOut(0.75)).toBe(0.875);
		expect(tabCollectionEaseInOut(1)).toBe(1);
	});

	it('places newly added children directly at target bounds', () => {
		const manager = new TabCollectionAnimatingLayoutManager();
		manager.onViewAdded('first');
		manager.recalculateTarget(stacked(['first']));
		expect(manager.currentFrame.layout.children[0]?.bounds).toEqual({
			x: 0,
			y: 0,
			width: 100,
			height: 20,
		});
	});
	it('keeps animated preferred height stable while equal-height children swap', () => {
		const manager = new TabCollectionAnimatingLayoutManager({
			animateHostSize: true,
		});
		manager.setChildren(['first', 'second']);
		manager.recalculateTarget(stacked(['first', 'second']));
		manager.complete();
		manager.recalculateTarget(stacked(['second', 'first']));
		for (const progress of [0, 0.1, 0.25, 0.5, 0.75, 0.9]) {
			manager.setProgress(progress);
			expect(
				manager.getPreferredSize({
					width: 100,
					height: 40,
				}).height,
			).toBe(40);
		}
		expect(manager.setProgress(0.5).layout.children).toEqual([
			{
				id: 'second',
				visible: true,
				bounds: {
					x: 0,
					y: 10,
					width: 100,
					height: 20,
				},
			},
			{
				id: 'first',
				visible: true,
				bounds: {
					x: 0,
					y: 10,
					width: 100,
					height: 20,
				},
			},
		]);
	});
	it('animates reparented children from their previous collection bounds', () => {
		const manager = new TabCollectionAnimatingLayoutManager();
		manager.animateAndReparentView('moved', {
			x: 20,
			y: 60,
			width: 100,
			height: 20,
		});
		manager.recalculateTarget(stacked(['moved']));
		expect(manager.currentFrame.layout.children[0]?.bounds).toEqual({
			x: 20,
			y: 60,
			width: 100,
			height: 20,
		});
		expect(manager.setProgress(0.5).layout.children[0]?.bounds).toEqual({
			x: 10,
			y: 30,
			width: 100,
			height: 20,
		});
	});
	it('supports source-directed animate-out for a reparented orphan', () => {
		const manager = new TabCollectionAnimatingLayoutManager();
		manager.setSourceLayoutInfo('orphan', {
			animationAxis: TabCollectionAnimationAxis.Horizontal,
			animationDirection: TabCollectionAnimationDirection.StartToEnd,
		});
		manager.animateAndReparentView('orphan', {
			x: 10,
			y: 30,
			width: 80,
			height: 20,
		});
		manager.recalculateTarget({
			hostSize: {
				width: 100,
				height: 0,
			},
			children: [],
		});
		expect(manager.setProgress(0.5).layout.children[0]?.bounds).toEqual({
			x: 50,
			y: 30,
			width: 40,
			height: 20,
		});
	});
	it('snaps dragging and explicitly snap-targeted views', () => {
		const manager = new TabCollectionAnimatingLayoutManager({
			delegate: {
				isViewDragging: (id) => id === 'dragging',
				shouldSnapToTarget: (id) => id === 'snapped',
			},
		});
		manager.setChildren(['dragging', 'snapped']);
		manager.recalculateTarget(stacked(['dragging', 'snapped']));
		expect(manager.currentFrame.layout.children.map((child) => child.bounds)).toEqual([
			{
				x: 0,
				y: 0,
				width: 100,
				height: 20,
			},
			{
				x: 0,
				y: 20,
				width: 100,
				height: 20,
			},
		]);
	});
	it('snaps on host-width changes instead of animating horizontal bounds', () => {
		const manager = new TabCollectionAnimatingLayoutManager();
		manager.setChildren(['tab']);
		manager.recalculateTarget(stacked(['tab'], 100));
		manager.complete();
		manager.recalculateTarget(stacked(['tab'], 120));
		expect(manager.isAnimating).toBe(false);
		expect(manager.currentFrame.layout.hostSize.width).toBe(120);
		expect(manager.currentFrame.layout.children[0]?.bounds.width).toBe(120);
	});
	it('normalizes early retargeting and restarts late retargeting', () => {
		const manager = new TabCollectionAnimatingLayoutManager();
		manager.setChildren(['first', 'second']);
		manager.recalculateTarget(stacked(['first', 'second']));
		manager.complete();
		manager.recalculateTarget(stacked(['second', 'first']));
		const atQuarter = requireDefined(
			manager.setProgress(0.25).layout.children.find((child) => child.id === 'first'),
		).bounds.y;
		manager.recalculateTarget(stacked(['first', 'second']));
		expect(
			requireDefined(
				manager.setProgress(0.25).layout.children.find((child) => child.id === 'first'),
			).bounds.y,
		).toBe(atQuarter);
		manager.setProgress(0.9);
		manager.recalculateTarget(stacked(['second', 'first']));
		expect(
			requireDefined(manager.currentFrame.layout.children.find((child) => child.id === 'first'))
				.bounds.y,
		).toBeGreaterThan(0);
		expect(
			requireDefined(manager.setProgress(0.5).layout.children.find((child) => child.id === 'first'))
				.bounds.y,
		).toBeGreaterThan(0);
	});
	it('notifies the delegate exactly when a sequence completes', () => {
		const onAnimationEnded = vi.fn<() => void>();
		const manager = new TabCollectionAnimatingLayoutManager({
			delegate: {
				onAnimationEnded,
			},
		});
		manager.onViewAdded('tab');
		manager.recalculateTarget(stacked(['tab']));
		manager.complete();
		expect(onAnimationEnded).toHaveBeenCalledTimes(1);
	});
});
