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

import { DESKTOP_SSH_PROJECT_ENROLLMENT_FAILURE_MESSAGES } from './constants/desktop-ssh-project-enrollment-dialog-messages';
import { useDesktopSshProjectEnrollmentDialog } from './hooks/useDesktopSshProjectEnrollmentDialog';
import type { DesktopSshProjectEnrollmentDialogController } from './model/DesktopSshProjectEnrollmentController';
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
	const title = intl.formatMessage(
		state.kind === 'enrollment'
			? desktopSshProjectMessages.title
			: desktopSshProjectMessages.connectionTitle,
	);
	const description = intl.formatMessage(
		state.kind === 'enrollment'
			? desktopSshProjectMessages.description
			: desktopSshProjectMessages.connectionDescription,
	);
	let progressMessage = desktopSshProjectMessages.progressConnecting;
	switch (state.progress) {
		case 'committing-project':
			progressMessage = desktopSshProjectMessages.progressCommittingProject;
			break;
		case 'inspecting-project':
			progressMessage = desktopSshProjectMessages.progressInspectingProject;
			break;
		case 'recording-environment':
			progressMessage = desktopSshProjectMessages.progressRecordingEnvironment;
			break;
		case 'resolving-target':
			progressMessage = desktopSshProjectMessages.progressResolvingTarget;
			break;
		case 'connecting':
		case null:
			break;
	}
	const prompt = state.phase === 'prompt' ? state.prompt : null;
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) controller.close();
			}}
			open
		>
			<DialogContent
				closeLabel={intl.formatMessage(desktopSshProjectMessages.close)}
				data-slot='desktop-ssh-project-enrollment'
			>
				<DialogHeader>
					<DialogTitle>{title}</DialogTitle>
					<DialogDescription>{description}</DialogDescription>
				</DialogHeader>
				{state.kind === 'enrollment' && state.phase === 'editing' ? (
					<div className={styles['fields']}>
						<Field>
							<FieldLabel htmlFor='ssh-project-name'>
								{intl.formatMessage(desktopSshProjectMessages.fieldName)}
							</FieldLabel>
							<Input
								autoFocus
								id='ssh-project-name'
								placeholder={intl.formatMessage(desktopSshProjectMessages.fieldNamePlaceholder)}
								value={state.form.name}
								onChange={(event) => controller.updateForm({ name: event.currentTarget.value })}
							/>
						</Field>
						<Field>
							<FieldLabel htmlFor='ssh-project-destination'>
								{intl.formatMessage(desktopSshProjectMessages.fieldDestination)}
							</FieldLabel>
							<Input
								id='ssh-project-destination'
								placeholder={intl.formatMessage(
									desktopSshProjectMessages.fieldDestinationPlaceholder,
								)}
								value={state.form.destination}
								onChange={(event) =>
									controller.updateForm({ destination: event.currentTarget.value })
								}
							/>
						</Field>
						<Field>
							<FieldLabel htmlFor='ssh-project-configuration-file'>
								{intl.formatMessage(desktopSshProjectMessages.fieldConfigurationFilePath)}
							</FieldLabel>
							<Input
								id='ssh-project-configuration-file'
								placeholder={intl.formatMessage(
									desktopSshProjectMessages.fieldConfigurationFilePathPlaceholder,
								)}
								value={state.form.configurationFilePath}
								onChange={(event) =>
									controller.updateForm({
										configurationFilePath: event.currentTarget.value,
									})
								}
							/>
							<FieldDescription>
								{intl.formatMessage(
									desktopSshProjectMessages.fieldConfigurationFilePathDescription,
								)}
							</FieldDescription>
						</Field>
						<Field>
							<FieldLabel htmlFor='ssh-project-path'>
								{intl.formatMessage(desktopSshProjectMessages.fieldProjectPath)}
							</FieldLabel>
							<Input
								id='ssh-project-path'
								placeholder={intl.formatMessage(
									desktopSshProjectMessages.fieldProjectPathPlaceholder,
								)}
								value={state.form.projectPath}
								onChange={(event) =>
									controller.updateForm({ projectPath: event.currentTarget.value })
								}
							/>
							<FieldDescription>
								{intl.formatMessage(desktopSshProjectMessages.fieldProjectPathDescription)}
							</FieldDescription>
						</Field>
					</div>
				) : null}
				{state.phase === 'working' ? (
					<p aria-live='polite' className={styles['progress']}>
						{intl.formatMessage(progressMessage)}
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
						<strong>{intl.formatMessage(desktopSshProjectMessages.failureTitle)}</strong>
						<span>
							{intl.formatMessage(DESKTOP_SSH_PROJECT_ENROLLMENT_FAILURE_MESSAGES[state.failure])}
						</span>
					</div>
				) : null}
				{state.phase === 'prompt' ? null : (
					<DialogFooter>
						<Button variant='outline' onClick={() => controller.close()}>
							{intl.formatMessage(desktopSshProjectMessages.cancel)}
						</Button>
						{state.phase === 'editing' && state.kind === 'enrollment' ? (
							<Button disabled={!state.canSubmit} onClick={() => controller.submit()}>
								{intl.formatMessage(desktopSshProjectMessages.connect)}
							</Button>
						) : null}
						{state.phase === 'failed' ? (
							<Button onClick={() => controller.retry()}>
								{intl.formatMessage(desktopSshProjectMessages.retry)}
							</Button>
						) : null}
					</DialogFooter>
				)}
			</DialogContent>
		</Dialog>
	);
}
