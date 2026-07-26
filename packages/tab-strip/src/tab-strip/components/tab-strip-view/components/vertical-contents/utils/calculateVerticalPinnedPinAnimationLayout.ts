import { LAYOUT_CONSTANTS } from '../../../../../layout/layout-constants';
import {
	calculatePinnedTabContainerLayout,
	type PinnedTabContainerLayoutInput,
} from '../../../../../layout/pinned-tab-container-layout';
import type { ChildLayout, ProposedLayout } from '../../../../../layout/proposed-layout';
import type { VerticalPinAnimationEntry } from './buildVerticalPinAnimationEntries';

function tween(value: number, start: number, target: number): number {
	return Math.round(start + (target - start) * value);
}

export function calculateVerticalPinnedPinAnimationLayout({
	children,
	...layoutInput
}: Omit<PinnedTabContainerLayoutInput, 'children'> & {
	children: readonly VerticalPinAnimationEntry[];
}): ProposedLayout {
	const calculate = (entries: readonly VerticalPinAnimationEntry[]) =>
		calculatePinnedTabContainerLayout({
			...layoutInput,
			children: entries.map((entry) => ({
				id: entry.key,
				preferredSize: {
					width: LAYOUT_CONSTANTS.verticalTabMinWidth,
					height: LAYOUT_CONSTANTS.verticalTabPinnedHeight,
				},
			})),
		});
	const previous = calculate(children.filter((entry) => entry.phase !== 'destination'));
	const target = calculate(children.filter((entry) => entry.phase !== 'source'));
	const previousById = new Map(previous.children.map((child) => [child.id, child]));
	const targetById = new Map(target.children.map((child) => [child.id, child]));
	const activeProgress = children
		.filter((entry) => entry.phase !== 'stable')
		.reduce((progress, entry) => Math.max(progress, entry.progress), 0);

	const animatedChildren: ChildLayout[] = [];
	for (const entry of children) {
		const start = previousById.get(entry.key);
		const end = targetById.get(entry.key);
		if (start && end) {
			animatedChildren.push({
				id: entry.key,
				visible: true,
				bounds: {
					x: tween(activeProgress, start.bounds.x, end.bounds.x),
					y: tween(activeProgress, start.bounds.y, end.bounds.y),
					width: tween(activeProgress, start.bounds.width, end.bounds.width),
					height: tween(activeProgress, start.bounds.height, end.bounds.height),
				},
			});
			continue;
		}
		const endpoint = end ?? start;
		if (!endpoint) continue;
		animatedChildren.push({
			id: entry.key,
			visible: true,
			bounds: {
				...endpoint.bounds,
				height: Math.round(endpoint.bounds.height * entry.mainAxisScale),
			},
		});
	}

	return {
		hostSize: {
			width: tween(activeProgress, previous.hostSize.width, target.hostSize.width),
			height: tween(activeProgress, previous.hostSize.height, target.hostSize.height),
		},
		children: animatedChildren,
	};
}
