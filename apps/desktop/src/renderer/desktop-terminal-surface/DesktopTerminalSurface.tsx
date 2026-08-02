import type { WorkbenchTerminalSurfaceProps } from '@orchbee/workbench';
import type { ReactNode } from 'react';

import styles from './DesktopTerminalSurface.module.css';
import { useDesktopTerminalSurface } from './hooks/useDesktopTerminalSurface';

export function DesktopTerminalSurface({ presentation }: WorkbenchTerminalSurfaceProps): ReactNode {
	const view = useDesktopTerminalSurface(presentation);
	return (
		<div
			className={styles['terminalSurface']}
			data-terminal-presentation-id={presentation.presentationId}
			data-terminal-projection-state={view.projectionState}
			ref={view.root}
		>
			{view.failureMessage === null ? null : (
				<div className={styles['status']} role='alert'>
					{view.failureMessage}
				</div>
			)}
		</div>
	);
}
