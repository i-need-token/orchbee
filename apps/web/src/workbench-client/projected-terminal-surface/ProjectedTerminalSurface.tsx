import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { WorkbenchTerminalSurfaceProps } from '@orchbee/workbench';
import type { ReactNode } from 'react';

import styles from './ProjectedTerminalSurface.module.css';
import { ProjectedTerminalImagePreviewCard } from './components/projected-terminal-image-preview-card/ProjectedTerminalImagePreviewCard';
import { ProjectedTerminalLaunchFailure } from './components/projected-terminal-launch-failure/ProjectedTerminalLaunchFailure';
import { ProjectedTerminalMobileInputBar } from './components/projected-terminal-mobile-input-bar/ProjectedTerminalMobileInputBar';
import { ProjectedTerminalPasteReviewDialog } from './components/projected-terminal-paste-review-dialog/ProjectedTerminalPasteReviewDialog';
import { ProjectedTerminalViewOnly } from './components/projected-terminal-view-only/ProjectedTerminalViewOnly';
import { useProjectedTerminalSurface } from './hooks/useProjectedTerminalSurface';

export function ProjectedTerminalSurface({
	actionRuntime,
	actionScope,
	admitBellEffect,
	autoFocus,
	bindActionContextProvider,
	bindBroadcastInputSink,
	closeTerminal,
	createTab,
	focusRequest,
	onPresentationReady,
	onTerminalEdit,
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
		closeTerminal,
		createTab,
		focusRequest,
		onPresentationReady,
		onTerminalEdit,
		presentationMode,
	});
	const intl = useOrchbeeIntl();
	const terminalShown = !view.stopped && view.launchFailure === null;
	return (
		<>
			<div
				className={styles['terminalSurface']}
				data-projected-terminal-close-activity={presentation.closeActivity}
				data-projected-terminal-mobile-controls={terminalShown ? 'visible' : 'hidden'}
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
				{view.launchFailure === null ? null : (
					<ProjectedTerminalLaunchFailure
						failure={view.launchFailure}
						onClose={view.closeTerminal}
						onCreateTab={view.createTab}
						rootRef={view.launchFailureRoot}
					/>
				)}
				{view.imagePreview === null ? null : (
					<ProjectedTerminalImagePreviewCard preview={view.imagePreview} />
				)}
				{terminalShown && view.controlPresentation !== 'none' ? (
					<ProjectedTerminalViewOnly
						onTakeControl={view.takeControl}
						taking={view.controlPresentation === 'taking-control'}
					/>
				) : null}
				{terminalShown ? (
					<ProjectedTerminalMobileInputBar
						onKey={view.sendMobileKey}
						onKeyboardToggle={view.toggleMobileKeyboard}
						onModifierToggle={view.toggleMobileModifier}
						state={view.mobileInputState}
					/>
				) : null}
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
