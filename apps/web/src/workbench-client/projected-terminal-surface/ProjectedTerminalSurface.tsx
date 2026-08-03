import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { WorkbenchTerminalSurfaceProps } from '@orchbee/workbench';
import type { ReactNode } from 'react';

import styles from './ProjectedTerminalSurface.module.css';
import { useProjectedTerminalSurface } from './hooks/useProjectedTerminalSurface';

export function ProjectedTerminalSurface({
	presentation,
}: WorkbenchTerminalSurfaceProps): ReactNode {
	const view = useProjectedTerminalSurface(presentation);
	const intl = useOrchbeeIntl();
	return (
		<div
			className={styles['terminalSurface']}
			data-terminal-presentation-id={presentation.presentationId}
			data-terminal-projection-state={view.projectionState}
			ref={view.root}
		>
			{view.stopped ? (
				<div className={styles['stopped']} data-slot='projected-terminal-stopped'>
					<p>{intl.formatMessage(projectedTerminalMessages.sessionStopped)}</p>
					<Button data-slot='projected-terminal-start-new-session' onClick={view.startNewSession}>
						{intl.formatMessage(projectedTerminalMessages.startNewSession)}
					</Button>
				</div>
			) : null}
			{view.failureMessage === null ? null : (
				<div className={styles['status']} role='alert'>
					{view.failureMessage}
				</div>
			)}
		</div>
	);
}
