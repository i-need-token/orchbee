import { requireDefined } from '../../common';
import { describe, expect, it, vi } from 'vitest';
import type { ProposedLayout } from '../proposed-layout';
import {
	TabCollectionAnimatingLayoutManager,
	TabCollectionAnimationAxis,
	TabCollectionAnimationDirection,
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
	it('animates newly added children from empty vertical bounds', () => {
		const manager = new TabCollectionAnimatingLayoutManager();
		manager.onViewAdded('first');
		manager.recalculateTarget(stacked(['first']));
		expect(manager.currentFrame.layout.children[0]?.bounds).toEqual({
			x: 0,
			y: 0,
			width: 100,
			height: 0,
		});
		expect(manager.setProgress(0.5).layout.children[0]?.bounds.height).toBe(10);
		expect(manager.setProgress(1).layout.children[0]?.bounds).toEqual({
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
	it('retains a pending-delete child until the removal animation completes', () => {
		const manager = new TabCollectionAnimatingLayoutManager({
			delegate: {
				shouldAnimateOpacityForAddAndRemove: () => true,
			},
		});
		manager.setChildren(['first', 'second']);
		manager.recalculateTarget(stacked(['first', 'second']));
		manager.complete();
		manager.animateAndDestroyChildView('second');
		manager.recalculateTarget(stacked(['first']));
		const halfway = manager.setProgress(0.5);
		expect(halfway.layout.children.find((child) => child.id === 'second')?.bounds.height).toBe(10);
		expect(halfway.opacityById.get('second')).toBe(0.5);
		expect(manager.complete()).toEqual(['second']);
		expect(manager.currentFrame.layout.children.map((child) => child.id)).toEqual(['first']);
	});
	it('uses horizontal add/remove bounds when configured', () => {
		const manager = new TabCollectionAnimatingLayoutManager({
			animationAxis: TabCollectionAnimationAxis.Horizontal,
		});
		manager.onViewAdded('tab');
		manager.recalculateTarget({
			hostSize: {
				width: 100,
				height: 20,
			},
			children: [
				{
					id: 'tab',
					visible: true,
					bounds: {
						x: 10,
						y: 0,
						width: 80,
						height: 20,
					},
				},
			],
		});
		expect(manager.currentFrame.layout.children[0]?.bounds).toEqual({
			x: 10,
			y: 0,
			width: 0,
			height: 20,
		});
		expect(manager.setProgress(0.5).layout.children[0]?.bounds.width).toBe(40);
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
				shouldSnapToTarget: (id) => id === 'split',
			},
		});
		manager.setChildren(['dragging', 'split']);
		manager.recalculateTarget(stacked(['dragging', 'split']));
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
