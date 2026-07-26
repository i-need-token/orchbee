import * as React from 'react';
import type { CSSProperties } from 'react';

import type { PanelDragGhost } from '../../types/panel-workspace-view';
import styles from './WorkbenchPanelDragGhost.module.css';

interface WorkbenchPanelDragGhostProps {
	readonly ghost: PanelDragGhost;
}

export function WorkbenchPanelDragGhost({ ghost }: WorkbenchPanelDragGhostProps): React.ReactNode {
	return (
		<div
			aria-hidden='true'
			className={styles['root']}
			data-slot='workbench-panel-drag-ghost'
			style={
				{
					'--ob-comp-workbench-panel-drag-block-start': `${ghost.blockStart}px`,
					'--ob-comp-workbench-panel-drag-height': `${ghost.height}px`,
					'--ob-comp-workbench-panel-drag-inline-start': `${ghost.inlineStart}px`,
					'--ob-comp-workbench-panel-drag-scale': ghost.scale,
					'--ob-comp-workbench-panel-drag-source-height': `${ghost.sourceHeight}px`,
					'--ob-comp-workbench-panel-drag-source-width': `${ghost.sourceWidth}px`,
					'--ob-comp-workbench-panel-drag-width': `${ghost.width}px`,
				} satisfies CSSProperties
			}
		>
			<div
				className={styles['snapshot']}
				data-slot='workbench-panel-drag-snapshot'
				ref={ghost.mountSnapshot}
			/>
		</div>
	);
}
