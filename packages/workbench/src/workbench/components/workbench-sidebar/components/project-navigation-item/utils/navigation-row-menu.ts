import type { KeyboardEvent } from 'react';

import { matchesNavigationMenuShortcut } from './navigation-menu-shortcuts';

export function findNavigationRowMenuTrigger(row: HTMLElement): HTMLButtonElement | null {
	const trigger = row.querySelector<HTMLButtonElement>('button[data-navigation-menu-trigger]');
	return trigger?.isConnected === true &&
		!trigger.disabled &&
		trigger.getAttribute('aria-disabled') !== 'true'
		? trigger
		: null;
}

export function openNavigationRowMenuFromKeyboard(event: KeyboardEvent<HTMLDivElement>): void {
	const NodeConstructor = event.currentTarget.ownerDocument.defaultView?.Node;
	if (
		event.defaultPrevented ||
		!matchesNavigationMenuShortcut(event.nativeEvent) ||
		NodeConstructor === undefined ||
		!(event.target instanceof NodeConstructor) ||
		!event.currentTarget.contains(event.target) ||
		event.currentTarget.hasAttribute('data-popup-open')
	) {
		return;
	}
	const trigger = findNavigationRowMenuTrigger(event.currentTarget);
	if (trigger === null || trigger.getAttribute('aria-expanded') === 'true') return;
	event.preventDefault();
	trigger.focus({ preventScroll: true });
	if (trigger.isConnected) trigger.click();
}
