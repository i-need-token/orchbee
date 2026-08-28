import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { WorkbenchTerminalSurfaceProps } from '@orchbee/workbench';
import type { ReactNode } from 'react';

import styles from './ProjectedTerminalSurface.module.css';
import { ProjectedTerminalPasteReviewDialog } from './components/projected-terminal-paste-review-dialog/ProjectedTerminalPasteReviewDialog';
import { useProjectedTerminalSurface } from './hooks/useProjectedTerminalSurface';

export function ProjectedTerminalSurface({
	actionRuntime,
	actionScope,
	admitBellEffect,
	autoFocus,
	bindActionContextProvider,
	bindBroadcastInputSink,
	onPresentationReady,
	presentation,
	presentationMode,
}: WorkbenchTerminalSurfaceProps): ReactNode {
	const view = useProjectedTerminalSurface(presentation, {
		actionRuntime,
		actionScope,
		admitBellEffect,
		autoFocus,
		bindActionContextProvider,
		bindBroadcastInputSink,
		onPresentationReady,
		presentationMode,
	});
	const intl = useOrchbeeIntl();
	return (
		<>
			<div
				className={styles['terminalSurface']}
				data-projected-terminal-close-activity={presentation.closeActivity}
				data-terminal-presentation-id={presentation.presentationId}
				data-terminal-projection-state={view.projectionState}
				onDragOver={view.handleFileDragOver}
				onDrop={view.handleFileDrop}
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
					<div data-projected-terminal-status='true' role='alert'>
						<span>{view.failureMessage}</span>
						{view.recoveryAvailable ? (
							<Button
								data-slot='projected-terminal-retry-connection'
								onClick={view.retryProjection}
								variant='outline'
							>
								{intl.formatMessage(projectedTerminalMessages.retryConnection)}
							</Button>
						) : null}
					</div>
				)}
			</div>
			{view.pasteReview === null ? null : (
				<ProjectedTerminalPasteReviewDialog
					onApprove={view.approvePaste}
					onCancel={view.cancelPaste}
					review={view.pasteReview}
				/>
			)}
		</>
	);
}
