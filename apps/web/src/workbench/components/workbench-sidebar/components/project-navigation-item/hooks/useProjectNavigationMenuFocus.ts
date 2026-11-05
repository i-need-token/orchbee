import { useMemoizedFn } from '@orchbee/foundation/react';
import type { ContextMenu } from '@orchbee/ui/components';
import * as React from 'react';

import { findNavigationRowMenuTrigger } from '../utils/navigation-row-menu';

type MenuChangeDetails = Parameters<
	NonNullable<React.ComponentProps<typeof ContextMenu>['onOpenChange']>
>[1];

interface NavigationMenuReturnTarget {
	readonly previousFocus: Element | null;
	readonly trigger: HTMLButtonElement;
	readonly restore: boolean;
}

export interface ProjectNavigationMenuFocus {
	readonly onOpenChange: (
		workspaceId: string | null,
		open: boolean,
		details: MenuChangeDetails,
	) => void;
	readonly finalFocus: (workspaceId: string | null) => HTMLButtonElement | false;
}

export function useProjectNavigationMenuFocus(): ProjectNavigationMenuFocus {
	const targets = React.useRef(new Map<string | null, NavigationMenuReturnTarget>());
	const onOpenChange = useMemoizedFn(
		(workspaceId: string | null, open: boolean, details: MenuChangeDetails): void => {
			if (open) {
				const target = details.event.target;
				const row =
					target instanceof Element
						? target.closest<HTMLElement>('[data-slot="context-menu-trigger"]')
						: null;
				const trigger = row === null ? null : findNavigationRowMenuTrigger(row);
				if (trigger === null) targets.current.delete(workspaceId);
				else
					targets.current.set(workspaceId, {
						previousFocus: trigger.ownerDocument.activeElement,
						restore: true,
						trigger,
					});
				return;
			}
			const captured = targets.current.get(workspaceId);
			if (captured === undefined) return;
			targets.current.set(workspaceId, {
				previousFocus: captured.previousFocus,
				restore: !['outside-press', 'focus-out', 'sibling-open'].includes(details.reason),
				trigger: captured.trigger,
			});
		},
	);
	const finalFocus = useMemoizedFn((workspaceId: string | null): HTMLButtonElement | false => {
		const captured = targets.current.get(workspaceId);
		targets.current.delete(workspaceId);
		if (
			captured === undefined ||
			!captured.restore ||
			!captured.trigger.isConnected ||
			captured.trigger.disabled ||
			captured.trigger.getAttribute('aria-disabled') === 'true'
		)
			return false;
		const { ownerDocument } = captured.trigger;
		const active = ownerDocument.activeElement;
		if (
			ownerDocument.querySelector('[role="dialog"], [role="alertdialog"]') !== null ||
			(active !== null &&
				active !== ownerDocument.body &&
				active !== captured.trigger &&
				active !== captured.previousFocus &&
				active.closest('[role="menu"]') === null)
		)
			return false;
		return captured.trigger;
	});

	return { finalFocus, onOpenChange };
}
