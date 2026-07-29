import { useMemoizedFn } from '@orchbee/foundation/react';
import { type TransitionEvent as ReactTransitionEvent, useEffect, useState } from 'react';
import type { TabGroupData } from '../../../../../model/tab-model';

interface VerticalGroupCollapseCompletion {
	readonly fullyCollapsedGroupIds: ReadonlySet<string>;
	readonly onGroupTabsTransitionEnd: (
		event: ReactTransitionEvent<HTMLDivElement>,
		groupId: string,
	) => void;
}

export function useVerticalGroupCollapseCompletion(
	groups: readonly Readonly<TabGroupData>[],
): VerticalGroupCollapseCompletion {
	const [fullyCollapsedGroupIds, setFullyCollapsedGroupIds] = useState<ReadonlySet<string>>(
		() => new Set(groups.filter((group) => group.collapsed).map((group) => group.id)),
	);

	useEffect(() => {
		const collapsedGroupIds = new Set(
			groups.filter((group) => group.collapsed).map((group) => group.id),
		);
		const reduceMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
		setFullyCollapsedGroupIds((current) => {
			const next = new Set([...current].filter((groupId) => collapsedGroupIds.has(groupId)));
			if (reduceMotion) collapsedGroupIds.forEach((groupId) => next.add(groupId));
			if (next.size === current.size && [...next].every((groupId) => current.has(groupId))) {
				return current;
			}
			return next;
		});
	}, [groups]);

	const onGroupTabsTransitionEnd = useMemoizedFn(
		(event: ReactTransitionEvent<HTMLDivElement>, groupId: string): void => {
			if (event.target !== event.currentTarget || event.propertyName !== 'grid-template-rows') {
				return;
			}
			const group = groups.find((candidate) => candidate.id === groupId);
			if (!group?.collapsed) return;
			setFullyCollapsedGroupIds((current) => {
				if (current.has(groupId)) return current;
				const next = new Set(current);
				next.add(groupId);
				return next;
			});
		},
	);

	return {
		fullyCollapsedGroupIds,
		onGroupTabsTransitionEnd,
	};
}
