import type * as React from 'react';

/**
 * Esc that no menu, field or drag inside the surface has already taken, which closes a surface
 * over the content: the Sidebar or the File Bar opened over it, or the Tab list.
 */
export function isUnclaimedEscape(event: React.KeyboardEvent<HTMLElement>): boolean {
	return (
		event.key === 'Escape' &&
		!event.defaultPrevented &&
		event.target instanceof Node &&
		event.currentTarget.contains(event.target) &&
		event.currentTarget.querySelector('[data-dragging]') === null
	);
}
