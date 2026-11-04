import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { WorkbenchTerminalSurfaceProps } from '../../workbench';
import type { ReactNode } from 'react';

import styles from './ProjectedTerminalSurface.module.css';
import { ProjectedTerminalLaunchFailure } from './components/projected-terminal-launch-failure/ProjectedTerminalLaunchFailure';
import { ProjectedTerminalMobileInputBar } from './components/projected-terminal-mobile-input-bar/ProjectedTerminalMobileInputBar';
import { ProjectedTerminalPasteReviewDialog } from './components/projected-terminal-paste-review-dialog/ProjectedTerminalPasteReviewDialog';
import { ProjectedTerminalPersistentStatus } from './components/projected-terminal-persistent-status/ProjectedTerminalPersistentStatus';
import { useProjectedTerminalSurface } from './hooks/useProjectedTerminalSurface';

export function ProjectedTerminalSurface({
	actionRuntime,
	actionScope,
	admitBellEffect,
	autoFocus,
	bindActionContextProvider,
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
		closeTerminal,
		createTab,
		focusRequest,
		onPresentationReady,
		onTerminalEdit,
		presentationMode,
	});
	const intl = useOrchbeeIntl();
	// An ended terminal keeps its last frame but takes no input and has no control to take.
	const terminalShown = view.launchFailure === null && view.terminalOutcome === null;
	return (
		<>
			<div
				className={styles['terminalSurface']}
				data-projected-terminal-close-activity={presentation.closeActivity}
				data-projected-terminal-control={terminalShown ? view.controlPresentation : 'none'}
				data-projected-terminal-mobile-controls={terminalShown ? 'visible' : 'hidden'}
				data-terminal-presentation-id={presentation.presentationId}
				data-terminal-projection-state={view.projectionState}
				ref={view.root}
			>
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
				{view.terminalOutcome === null ? null : (
					<ProjectedTerminalPersistentStatus
						state={{ kind: 'ended', onCreateTab: view.createTab, outcome: view.terminalOutcome }}
					/>
				)}
				{terminalShown && view.controlPresentation !== 'none' ? (
					<ProjectedTerminalPersistentStatus
						state={{
							cropped: view.liveCropped,
							kind: 'view-only',
							onTakeControl: view.takeControl,
							taking: view.controlPresentation === 'taking-control',
						}}
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
