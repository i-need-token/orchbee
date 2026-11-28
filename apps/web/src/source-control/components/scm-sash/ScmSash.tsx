import * as React from 'react';

import styles from './ScmSash.module.css';

interface ScmSashProps {
	/** The pointer moved this far down from where the drag began, in CSS pixels. */
	readonly onDrag: (offset: number) => void;
	readonly onDragEnd: () => void;
	readonly onDragStart: () => void;
}

/**
 * VS Code's sash between two expanded panes: a thin band on their boundary that resizes them by
 * dragging. It answers the pointer only, as VS Code's sashes do.
 */
export function ScmSash({ onDrag, onDragEnd, onDragStart }: ScmSashProps): React.ReactNode {
	const origin = React.useRef<number | null>(null);
	const end = (event: React.PointerEvent<HTMLDivElement>): void => {
		if (origin.current === null) return;
		origin.current = null;
		if (event.currentTarget.hasPointerCapture(event.pointerId)) {
			event.currentTarget.releasePointerCapture(event.pointerId);
		}
		onDragEnd();
	};
	return (
		<div
			aria-hidden='true'
			className={styles['sash']}
			data-slot='scm-sash'
			onPointerCancel={end}
			onPointerDown={(event) => {
				if (event.button !== 0) return;
				event.preventDefault();
				event.currentTarget.setPointerCapture(event.pointerId);
				origin.current = event.clientY;
				onDragStart();
			}}
			onPointerMove={(event) => {
				if (origin.current !== null) onDrag(event.clientY - origin.current);
			}}
			onPointerUp={end}
		/>
	);
}
