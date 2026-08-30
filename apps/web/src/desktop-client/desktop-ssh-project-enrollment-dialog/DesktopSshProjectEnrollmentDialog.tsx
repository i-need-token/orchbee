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
	FieldError,
	FieldLabel,
	Input,
	Kbd,
	Spinner,
} from '@orchbee/ui/components';
import type { FormEvent, ReactNode } from 'react';

import { RemoteDirectoryBrowser } from '../../remote-directory-browser';
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
	const { close, destinationInputReference, destinationInvalid, state, submitDestination } =
		useDesktopSshProjectEnrollmentDialog(controller);
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
				if (!open) close();
			}}
			open
		>
			<DialogContent
				className={choosingDirectory ? styles['directoryDialog'] : undefined}
				closeLabel={intl.formatMessage(desktopSshProjectMessages.close)}
				data-slot='desktop-ssh-project-enrollment'
			>
				<DialogHeader className={styles['header']}>
					<DialogTitle className={choosingDirectory ? undefined : styles['title']}>
						{title}
					</DialogTitle>
					<DialogDescription
						className={
							state.phase === 'failed' ? styles['failureDescription'] : styles['description']
						}
						data-compact-typography={choosingDirectory ? undefined : true}
						role={state.phase === 'failed' ? 'alert' : undefined}
					>
						{description}
					</DialogDescription>
				</DialogHeader>
				{directoryOperation && state.phase === 'editing' ? (
					<form className={styles['form']} noValidate onSubmit={submitDestination}>
						<Field data-invalid={destinationInvalid}>
							<FieldLabel className={styles['controlText']} htmlFor='ssh-project-destination'>
								{intl.formatMessage(desktopSshProjectMessages.fieldDestination)}
							</FieldLabel>
							<Input
								aria-describedby={
									destinationInvalid
										? 'ssh-project-destination-description ssh-project-destination-error'
										: 'ssh-project-destination-description'
								}
								aria-invalid={destinationInvalid}
								autoCapitalize='none'
								autoComplete='off'
								autoCorrect='off'
								autoFocus
								className={styles['destinationInput']}
								id='ssh-project-destination'
								placeholder={intl.formatMessage(
									desktopSshProjectMessages.fieldDestinationPlaceholder,
								)}
								ref={destinationInputReference}
								required
								spellCheck={false}
								value={state.form.destination}
								onChange={(event) =>
									controller.updateForm({ destination: event.currentTarget.value })
								}
							/>
							<FieldDescription
								className={styles['supportingText']}
								id='ssh-project-destination-description'
							>
								{intl.formatMessage(desktopSshProjectMessages.fieldDestinationDescription)}
							</FieldDescription>
							{destinationInvalid ? (
								<FieldError className={styles['supportingText']} id='ssh-project-destination-error'>
									{intl.formatMessage(desktopSshProjectMessages.fieldDestinationError)}
								</FieldError>
							) : null}
						</Field>
						<DialogFooter>
							<Button
								className={styles['controlText']}
								type='button'
								variant='outline'
								onClick={close}
							>
								{intl.formatMessage(desktopSshProjectMessages.cancel)}
								<Kbd aria-hidden='true' className={styles['shortcut']}>
									Esc
								</Kbd>
							</Button>
							<Button className={styles['controlText']} type='submit'>
								{intl.formatMessage(desktopSshProjectMessages.connect)}
								<Kbd aria-hidden='true' className={styles['shortcut']}>
									Enter
								</Kbd>
							</Button>
						</DialogFooter>
					</form>
				) : null}
				{choosingDirectory ? (
					<RemoteDirectoryBrowser
						busy={state.phase === 'working'}
						canNavigate={state.canNavigate}
						location={toRemoteDirectoryLocation(state.directory)}
						onCancel={close}
						onNavigate={(path) => controller.navigateDirectory(path)}
						onPathDraftChange={(path) => controller.updateDirectoryPath(path)}
						onSelect={() => controller.selectDirectory()}
						pathDraft={state.pathInput}
						sourceLabel={sshSourceLabel(state.form.destination)}
					/>
				) : null}
				{state.phase === 'working' && !choosingDirectory ? (
					<p aria-live='polite' aria-atomic='true' className={styles['progress']}>
						<Spinner aria-hidden='true' />
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
						<pre className={styles['promptText']}>{prompt.prompt}</pre>
						{prompt.kind === 'confirm' ? null : (
							<Field>
								<FieldLabel
									className={styles['controlText']}
									htmlFor={`ssh-project-response-${prompt.handle}`}
								>
									{intl.formatMessage(desktopSshProjectMessages.promptSecret)}
								</FieldLabel>
								<Input
									autoComplete='off'
									autoFocus
									className={styles['plainInput']}
									id={`ssh-project-response-${prompt.handle}`}
									name='response'
									required
									spellCheck={false}
									type='password'
								/>
							</Field>
						)}
						<DialogFooter>
							<Button
								autoFocus={prompt.kind === 'confirm'}
								className={styles['controlText']}
								type='button'
								variant='outline'
								onClick={close}
							>
								{intl.formatMessage(desktopSshProjectMessages.cancel)}
								<Kbd aria-hidden='true' className={styles['shortcut']}>
									Esc
								</Kbd>
							</Button>
							<Button className={styles['controlText']} type='submit'>
								{intl.formatMessage(
									prompt.kind === 'confirm'
										? desktopSshProjectMessages.trustHost
										: desktopSshProjectMessages.authenticate,
								)}
								{prompt.kind === 'confirm' ? null : (
									<Kbd aria-hidden='true' className={styles['shortcut']}>
										Enter
									</Kbd>
								)}
							</Button>
						</DialogFooter>
					</form>
				)}
				{(state.phase === 'working' || state.phase === 'failed') && !choosingDirectory ? (
					<DialogFooter>
						<Button className={styles['controlText']} variant='outline' onClick={close}>
							{intl.formatMessage(desktopSshProjectMessages.cancel)}
							<Kbd aria-hidden='true' className={styles['shortcut']}>
								Esc
							</Kbd>
						</Button>
						{state.phase === 'failed' ? (
							<Button className={styles['controlText']} onClick={() => controller.retry()}>
								{intl.formatMessage(desktopSshProjectMessages.retry)}
							</Button>
						) : null}
					</DialogFooter>
				) : null}
			</DialogContent>
		</Dialog>
	);
}
