import { desktopSshProjectMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	Button,
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	Field,
	FieldDescription,
	FieldLabel,
	Input,
} from '@orchbee/ui/components';
import type { FormEvent, ReactNode } from 'react';

import { RemoteDirectoryBrowser } from '../../remote-directory-browser';
import { DESKTOP_SSH_PROJECT_ENROLLMENT_FAILURE_MESSAGES } from './constants/desktop-ssh-project-enrollment-dialog-messages';
import { useDesktopSshProjectEnrollmentDialog } from './hooks/useDesktopSshProjectEnrollmentDialog';
import type { DesktopSshProjectEnrollmentDialogController } from './model/DesktopSshProjectEnrollmentController';
import {
	desktopSshProjectDialogDescriptionMessage,
	desktopSshProjectDialogTitleMessage,
	desktopSshProjectProgressMessage,
	sshSourceLabel,
	toRemoteDirectoryLocation,
} from './utils/desktop-ssh-project-enrollment-dialog-view';
import styles from './DesktopSshProjectEnrollmentDialog.module.css';

export interface DesktopSshProjectEnrollmentDialogProps {
	readonly controller: DesktopSshProjectEnrollmentDialogController;
}

export function DesktopSshProjectEnrollmentDialog({
	controller,
}: DesktopSshProjectEnrollmentDialogProps): ReactNode {
	const intl = useOrchbeeIntl();
	const state = useDesktopSshProjectEnrollmentDialog(controller);
	if (state.kind === 'closed') return null;
	const directoryOperation = state.kind === 'attachment' || state.kind === 'enrollment';
	const choosingDirectory =
		directoryOperation &&
		state.directory !== null &&
		(state.phase === 'directory' || state.phase === 'working');
	const title = intl.formatMessage(desktopSshProjectDialogTitleMessage(state, choosingDirectory));
	const description = intl.formatMessage(
		desktopSshProjectDialogDescriptionMessage(state, choosingDirectory),
	);
	const prompt = state.phase === 'prompt' ? state.prompt : null;
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) controller.close();
			}}
			open
		>
			<DialogContent
				className={choosingDirectory ? styles['directoryDialog'] : undefined}
				closeLabel={intl.formatMessage(desktopSshProjectMessages.close)}
				data-slot='desktop-ssh-project-enrollment'
			>
				<DialogHeader>
					<DialogTitle>{title}</DialogTitle>
					<DialogDescription>{description}</DialogDescription>
				</DialogHeader>
				{directoryOperation && state.phase === 'editing' ? (
					<form
						className={styles['fields']}
						onSubmit={(event: FormEvent<HTMLFormElement>) => {
							event.preventDefault();
							controller.submit();
						}}
					>
						<Field>
							<FieldLabel htmlFor='ssh-project-destination'>
								{intl.formatMessage(desktopSshProjectMessages.fieldDestination)}
							</FieldLabel>
							<Input
								autoFocus
								id='ssh-project-destination'
								placeholder={intl.formatMessage(
									desktopSshProjectMessages.fieldDestinationPlaceholder,
								)}
								value={state.form.destination}
								onChange={(event) =>
									controller.updateForm({ destination: event.currentTarget.value })
								}
							/>
							<FieldDescription>
								{intl.formatMessage(desktopSshProjectMessages.fieldDestinationDescription)}
							</FieldDescription>
						</Field>
						<DialogFooter>
							<Button type='button' variant='outline' onClick={() => controller.close()}>
								{intl.formatMessage(desktopSshProjectMessages.cancel)}
							</Button>
							<Button disabled={!state.canSubmit} type='submit'>
								{intl.formatMessage(desktopSshProjectMessages.connect)}
							</Button>
						</DialogFooter>
					</form>
				) : null}
				{choosingDirectory ? (
					<RemoteDirectoryBrowser
						busy={state.phase === 'working'}
						canNavigate={state.canNavigate}
						location={toRemoteDirectoryLocation(state.directory)}
						onCancel={() => controller.close()}
						onNavigate={(path) => controller.navigateDirectory(path)}
						onPathDraftChange={(path) => controller.updateDirectoryPath(path)}
						onSelect={() => controller.selectDirectory()}
						pathDraft={state.pathInput}
						sourceLabel={sshSourceLabel(state.form.destination)}
					/>
				) : null}
				{state.phase === 'working' && !choosingDirectory ? (
					<p aria-live='polite' className={styles['progress']}>
						{intl.formatMessage(desktopSshProjectProgressMessage(state))}
					</p>
				) : null}
				{prompt === null ? null : (
					<form
						key={prompt.handle}
						className={styles['prompt']}
						onSubmit={(event: FormEvent<HTMLFormElement>) => {
							event.preventDefault();
							let response = 'yes';
							if (prompt.kind !== 'confirm') {
								const value = new FormData(event.currentTarget).get('response');
								response = typeof value === 'string' ? value : '';
							}
							controller.resolvePrompt({ kind: 'submitted', response });
						}}
					>
						<div>
							<h3>
								{intl.formatMessage(
									prompt.kind === 'confirm'
										? desktopSshProjectMessages.promptHostTitle
										: desktopSshProjectMessages.promptAuthenticationTitle,
								)}
							</h3>
							<p className={styles['promptDescription']}>
								{intl.formatMessage(
									prompt.kind === 'confirm'
										? desktopSshProjectMessages.promptHostDescription
										: desktopSshProjectMessages.promptAuthenticationDescription,
								)}
							</p>
						</div>
						<pre className={styles['promptText']}>{prompt.prompt}</pre>
						{prompt.kind === 'confirm' ? null : (
							<Field>
								<FieldLabel htmlFor={`ssh-project-response-${prompt.handle}`}>
									{intl.formatMessage(desktopSshProjectMessages.promptResponse)}
								</FieldLabel>
								<Input
									autoFocus
									id={`ssh-project-response-${prompt.handle}`}
									name='response'
									required
									type='password'
								/>
							</Field>
						)}
						<DialogFooter>
							<Button type='button' variant='outline' onClick={() => controller.close()}>
								{intl.formatMessage(desktopSshProjectMessages.cancel)}
							</Button>
							<Button type='submit'>
								{intl.formatMessage(desktopSshProjectMessages.continue)}
							</Button>
						</DialogFooter>
					</form>
				)}
				{state.phase === 'failed' && state.failure !== null ? (
					<div className={styles['failure']} role='alert'>
						<strong>
							{intl.formatMessage(
								state.kind === 'environment-connection'
									? desktopSshProjectMessages.environmentConnectionFailureTitle
									: desktopSshProjectMessages.failureTitle,
							)}
						</strong>
						<span>
							{intl.formatMessage(DESKTOP_SSH_PROJECT_ENROLLMENT_FAILURE_MESSAGES[state.failure])}
						</span>
					</div>
				) : null}
				{(state.phase === 'working' || state.phase === 'failed') && !choosingDirectory ? (
					<DialogFooter>
						<Button variant='outline' onClick={() => controller.close()}>
							{intl.formatMessage(desktopSshProjectMessages.cancel)}
						</Button>
						{state.phase === 'failed' ? (
							<Button onClick={() => controller.retry()}>
								{intl.formatMessage(desktopSshProjectMessages.retry)}
							</Button>
						) : null}
					</DialogFooter>
				) : null}
			</DialogContent>
		</Dialog>
	);
}
