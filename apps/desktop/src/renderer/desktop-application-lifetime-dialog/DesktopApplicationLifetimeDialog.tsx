import { desktopApplicationLifetimeMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { useDesktopApplicationLifetimeDialog } from './hooks/useDesktopApplicationLifetimeDialog';
import type { DesktopApplicationLifetimeState } from './model/DesktopApplicationLifetimeCoordinator';
import type { DesktopApplicationLifetimeDialogController } from './types/desktop-application-lifetime-dialog';

interface DesktopApplicationLifetimeDialogProps {
	readonly controller: DesktopApplicationLifetimeDialogController;
}

export function DesktopApplicationLifetimeDialog({
	controller,
}: DesktopApplicationLifetimeDialogProps): ReactNode {
	const intl = useOrchbeeIntl();
	const state = useDesktopApplicationLifetimeDialog(controller);
	if (state.kind === 'idle') return null;

	if (state.kind === 'awaiting-choice') {
		return (
			<AlertDialog open>
				<AlertDialogContent data-slot='desktop-application-lifetime-dialog'>
					<AlertDialogHeader>
						<AlertDialogTitle>
							{intl.formatMessage(desktopApplicationLifetimeMessages.exitTitle)}
						</AlertDialogTitle>
						<AlertDialogDescription>
							{intl.formatMessage(desktopApplicationLifetimeMessages.exitDescription, {
								sessionCount: state.sessionCount,
							})}
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel
							data-lifetime-choice='cancel'
							onClick={() => controller.choose('cancel')}
						>
							{intl.formatMessage(desktopApplicationLifetimeMessages.cancel)}
						</AlertDialogCancel>
						<AlertDialogAction
							data-lifetime-choice='stop'
							variant='destructive'
							onClick={() => controller.choose('stop')}
						>
							{intl.formatMessage(desktopApplicationLifetimeMessages.stopAndQuit)}
						</AlertDialogAction>
						<AlertDialogAction
							autoFocus
							data-lifetime-choice='preserve'
							onClick={() => controller.choose('preserve')}
						>
							{intl.formatMessage(desktopApplicationLifetimeMessages.preserveAndQuit)}
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		);
	}

	if (state.kind === 'failed') {
		return (
			<AlertDialog open>
				<AlertDialogContent data-slot='desktop-application-lifetime-dialog'>
					<AlertDialogHeader>
						<AlertDialogTitle>
							{intl.formatMessage(desktopApplicationLifetimeMessages.failedTitle)}
						</AlertDialogTitle>
						<AlertDialogDescription>
							{intl.formatMessage(desktopApplicationLifetimeMessages.failedDescription)}
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						{state.retry === null ? null : (
							<AlertDialogAction data-lifetime-action='retry' onClick={() => controller.retry()}>
								{intl.formatMessage(desktopApplicationLifetimeMessages.retry)}
							</AlertDialogAction>
						)}
						<AlertDialogAction
							data-lifetime-action='quit-anyway'
							variant='destructive'
							onClick={() => controller.quitAnyway()}
						>
							{intl.formatMessage(desktopApplicationLifetimeMessages.quitAnyway)}
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		);
	}

	return (
		<AlertDialog open>
			<AlertDialogContent data-slot='desktop-application-lifetime-dialog'>
				<AlertDialogHeader>
					<AlertDialogTitle>
						{intl.formatMessage(desktopApplicationLifetimeMessages.progressTitle)}
					</AlertDialogTitle>
					<AlertDialogDescription>{progressMessage(state)}</AlertDialogDescription>
				</AlertDialogHeader>
			</AlertDialogContent>
		</AlertDialog>
	);

	function progressMessage(
		progressState: Extract<DesktopApplicationLifetimeState, { kind: 'exiting' | 'working' }>,
	): string {
		if (progressState.kind === 'exiting') {
			return intl.formatMessage(desktopApplicationLifetimeMessages.progressExiting);
		}
		if (progressState.progress.phase === 'stopping-terminals') {
			return intl.formatMessage(desktopApplicationLifetimeMessages.progressStopping, {
				completedCount: progressState.progress.completedCount,
				totalCount: progressState.progress.totalCount,
			});
		}
		if (
			progressState.progress.phase === 'draining-unreferenced' ||
			progressState.progress.phase === 'verifying-host'
		) {
			return intl.formatMessage(desktopApplicationLifetimeMessages.progressDraining);
		}
		return intl.formatMessage(desktopApplicationLifetimeMessages.progressFlushing);
	}
}
