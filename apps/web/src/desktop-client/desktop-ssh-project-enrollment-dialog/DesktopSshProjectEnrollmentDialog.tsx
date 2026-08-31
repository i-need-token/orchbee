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
import { type FormEvent, type KeyboardEvent, type ReactNode } from 'react';

import { RemoteDirectoryBrowser } from '../../remote-directory-browser';
import { useDesktopSshProjectEnrollmentDialog } from './hooks/useDesktopSshProjectEnrollmentDialog';
import type { DesktopSshProjectEnrollmentDialogController } from './model/DesktopSshProjectEnrollmentController';
import {
	desktopSshProjectDialogDescriptionMessage,
	desktopSshProjectDialogTitleMessage,
	desktopSshProjectConnectionDisplayTarget,
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
	const {
		cancel,
		close,
		connectDestination,
		destinationInputReference,
		destinationInvalid,
		destinationOptions,
		retryButtonReference,
		state,
		submitDestination,
	} = useDesktopSshProjectEnrollmentDialog(controller);
	if (state.kind === 'closed') return null;
	const directoryOperation = state.kind === 'attachment' || state.kind === 'enrollment';
	const choosingDirectory =
		directoryOperation &&
		state.directory !== null &&
		(state.phase === 'directory' || state.phase === 'working');
	const title = intl.formatMessage(desktopSshProjectDialogTitleMessage(state, choosingDirectory));
	const connectionDisplayTarget = desktopSshProjectConnectionDisplayTarget(state);
	let description = intl.formatMessage(
		desktopSshProjectDialogDescriptionMessage(state, choosingDirectory),
	);
	if (state.phase === 'failed' && connectionDisplayTarget !== null) {
		description =
			state.failure === 'connection'
				? intl.formatMessage(desktopSshProjectMessages.failureConnectionWithDestination, {
						destination: connectionDisplayTarget,
					})
				: `${description} ${intl.formatMessage(
						desktopSshProjectMessages.failureDestinationContext,
						{ destination: connectionDisplayTarget },
					)}`;
	}
	const prompt = state.phase === 'prompt' ? state.prompt : null;
	const authenticationPrompt = prompt?.kind === 'confirm' ? null : (prompt?.prompt ?? null);
	return (
		<Dialog
			onOpenChange={(open, eventDetails) => {
				if (open) return;
				if (eventDetails.reason === 'escape-key') cancel();
				else close();
			}}
			open
		>
			<DialogContent
				className={choosingDirectory ? styles['directoryDialog'] : undefined}
				closeLabel={intl.formatMessage(desktopSshProjectMessages.close)}
				data-slot='desktop-ssh-project-enrollment'
				onKeyDown={(event: KeyboardEvent<HTMLDivElement>) => {
					if (
						state.phase !== 'failed' ||
						event.key !== 'Enter' ||
						event.repeat ||
						event.altKey ||
						event.ctrlKey ||
						event.metaKey ||
						event.shiftKey ||
						event.nativeEvent.isComposing
					) {
						return;
					}
					const focusedButton =
						event.target instanceof Element ? event.target.closest('button') : null;
					if (focusedButton !== null && focusedButton !== retryButtonReference.current) return;
					event.preventDefault();
					controller.retry();
				}}
			>
				<DialogHeader className={styles['header']}>
					<DialogTitle className={styles['title']}>{title}</DialogTitle>
					{choosingDirectory ? null : (
						<DialogDescription
							className={styles['description']}
							data-authentication={authenticationPrompt === null ? undefined : true}
							data-compact-typography={choosingDirectory ? undefined : true}
							data-failure={state.phase === 'failed' ? true : undefined}
							role={state.phase === 'failed' ? 'alert' : undefined}
						>
							{authenticationPrompt === null ? (
								description
							) : (
								<>
									<code className={styles['authenticationPrompt']}>{authenticationPrompt}</code>
									<span>{description}</span>
								</>
							)}
						</DialogDescription>
					)}
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
							{destinationOptions.length > 0 ? (
								<ul className={styles['destinationCandidates']}>
									{destinationOptions.map((destination) => (
										<li key={destination}>
											<Button
												aria-label={destination}
												className={styles['destinationCandidate']}
												type='button'
												variant='ghost'
												onClick={() => connectDestination(destination)}
											>
												<span className={styles['destinationCandidateAddress']}>{destination}</span>
												<span aria-hidden='true' className={styles['destinationCandidateType']}>
													SSH
												</span>
											</Button>
										</li>
									))}
								</ul>
							) : null}
						</Field>
						<DialogFooter>
							<Button
								className={styles['controlText']}
								type='button'
								variant='outline'
								onClick={cancel}
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
						onCancel={cancel}
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
						<span className={styles['progressContent']}>
							{connectionDisplayTarget === null ? null : (
								<>
									<span>
										{intl.formatMessage(desktopSshProjectMessages.progressConnectionLabel)}
									</span>
									<code>{connectionDisplayTarget}</code>
									<span aria-hidden='true'>·</span>
								</>
							)}
							<span>{intl.formatMessage(desktopSshProjectProgressMessage(state))}</span>
						</span>
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
						{prompt.kind === 'confirm' ? (
							<pre className={styles['promptText']}>{prompt.prompt}</pre>
						) : (
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
								onClick={cancel}
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
						<Button className={styles['controlText']} variant='outline' onClick={cancel}>
							{intl.formatMessage(desktopSshProjectMessages.cancel)}
							<Kbd aria-hidden='true' className={styles['shortcut']}>
								Esc
							</Kbd>
						</Button>
						{state.phase === 'failed' ? (
							<Button
								className={styles['controlText']}
								ref={retryButtonReference}
								onClick={() => controller.retry()}
							>
								{intl.formatMessage(desktopSshProjectMessages.retry)}
								<Kbd aria-hidden='true' className={styles['shortcut']}>
									Enter
								</Kbd>
							</Button>
						) : null}
					</DialogFooter>
				) : null}
			</DialogContent>
		</Dialog>
	);
}
