import {
	desktopBrowserAccessMessages,
	type MessageDescriptor,
	useOrchbeeIntl,
} from '@orchbee/i18n';
import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
	Button,
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { DESKTOP_BROWSER_ACCESS_ERROR_MESSAGES } from './constants/desktop-browser-access-dialog-messages';
import { useDesktopBrowserAccessDialog } from './hooks/useDesktopBrowserAccessDialog';
import type { DesktopBrowserAccessDialogController } from './model/DesktopBrowserAccessController';
import styles from './DesktopBrowserAccessDialog.module.css';

interface DesktopBrowserAccessDialogProps {
	readonly controller: DesktopBrowserAccessDialogController;
	readonly finalFocus?: (() => HTMLElement | null) | undefined;
}

export function DesktopBrowserAccessDialog({
	controller,
	finalFocus,
}: DesktopBrowserAccessDialogProps): ReactNode {
	const intl = useOrchbeeIntl();
	const state = useDesktopBrowserAccessDialog(controller);
	const busy = state.pendingAction !== null;
	const runtime = state.snapshot.access;
	const personalAccessTokenConfigured = state.snapshot.personalAccessToken === 'configured';
	const ready = runtime.phase === 'ready' ? runtime : null;
	const addresses =
		ready === null
			? []
			: [
					{
						label: desktopBrowserAccessMessages.thisComputer,
						origin: ready.localOrigin,
					},
					...ready.networkOrigins.map((origin) => ({
						label: desktopBrowserAccessMessages.localNetwork,
						origin,
					})),
				];
	let statusMessage: MessageDescriptor = desktopBrowserAccessMessages.stoppedStatus;
	if (state.pendingAction?.startsWith('start-') === true || runtime.phase === 'starting') {
		statusMessage = desktopBrowserAccessMessages.startProgress;
	} else if (state.pendingAction === 'stop' || runtime.phase === 'stopping') {
		statusMessage = desktopBrowserAccessMessages.stopProgress;
	} else if (runtime.phase === 'ready') {
		statusMessage =
			runtime.mode === 'loopback'
				? desktopBrowserAccessMessages.loopbackStatus
				: desktopBrowserAccessMessages.localNetworkStatus;
	}
	let statusDetail: ReactNode = null;
	if (runtime.phase === 'stopped' && state.pendingAction === null) {
		statusDetail = (
			<span className={styles['description']}>
				{intl.formatMessage(desktopBrowserAccessMessages.stoppedDescription)}
			</span>
		);
	}

	let personalAccessTokenStatus = desktopBrowserAccessMessages.personalAccessTokenMissing;
	if (state.snapshot.personalAccessToken === 'configured') {
		personalAccessTokenStatus = desktopBrowserAccessMessages.personalAccessTokenConfigured;
	} else if (state.snapshot.personalAccessToken === 'unavailable') {
		personalAccessTokenStatus = desktopBrowserAccessMessages.personalAccessTokenUnavailable;
	}
	let personalAccessTokenActionMessage: MessageDescriptor =
		desktopBrowserAccessMessages.createPersonalAccessToken;
	if (state.pendingAction === 'issue-personal-access-token') {
		personalAccessTokenActionMessage =
			desktopBrowserAccessMessages.updatePersonalAccessTokenProgress;
	} else if (personalAccessTokenConfigured) {
		personalAccessTokenActionMessage = desktopBrowserAccessMessages.resetPersonalAccessToken;
	}

	return (
		<>
			<Dialog
				open={state.open}
				onOpenChange={(open) => {
					if (!open) controller.close();
				}}
			>
				<DialogContent
					className={styles['dialog']}
					closeLabel={intl.formatMessage(desktopBrowserAccessMessages.close)}
					data-slot='desktop-browser-access-dialog'
					finalFocus={finalFocus}
				>
					<DialogHeader className={styles['dialogHeader']}>
						<DialogTitle className={styles['dialogTitle']}>
							{intl.formatMessage(desktopBrowserAccessMessages.title)}
						</DialogTitle>
						<DialogDescription className={styles['dialogDescription']}>
							{intl.formatMessage(desktopBrowserAccessMessages.description)}
						</DialogDescription>
					</DialogHeader>

					<div className={styles['content']}>
						<section className={styles['status']} aria-live='polite'>
							<strong className={styles['statusTitle']}>{intl.formatMessage(statusMessage)}</strong>
							{statusDetail}
						</section>

						{state.error === null ? null : (
							<div className={styles['error']} role='alert'>
								{intl.formatMessage(DESKTOP_BROWSER_ACCESS_ERROR_MESSAGES[state.error])}
							</div>
						)}

						<section className={styles['personalAccessToken']}>
							<div className={styles['personalAccessTokenHeader']}>
								<div>
									<h3 className={styles['sectionTitle']}>
										{intl.formatMessage(desktopBrowserAccessMessages.personalAccessTokenTitle)}
									</h3>
									<p className={styles['personalAccessTokenDescription']}>
										{intl.formatMessage(
											desktopBrowserAccessMessages.personalAccessTokenDescription,
										)}
									</p>
								</div>
								<span
									className={styles['personalAccessTokenStatus']}
									data-status={state.snapshot.personalAccessToken}
								>
									{intl.formatMessage(personalAccessTokenStatus)}
								</span>
							</div>

							{state.issuedPersonalAccessToken === null ? null : (
								<div className={styles['issuedPersonalAccessToken']} role='status'>
									<strong>
										{intl.formatMessage(
											desktopBrowserAccessMessages.personalAccessTokenOneTimeTitle,
										)}
									</strong>
									<span className={styles['description']}>
										{intl.formatMessage(
											desktopBrowserAccessMessages.personalAccessTokenOneTimeDescription,
										)}
									</span>
									<code
										className={styles['personalAccessTokenValue']}
										data-slot='desktop-personal-access-token-value'
									>
										{state.issuedPersonalAccessToken}
									</code>
									<Button
										data-browser-access-action='copy-personal-access-token'
										disabled={busy}
										size='sm'
										variant='outline'
										onClick={() => controller.copyPersonalAccessToken()}
									>
										{intl.formatMessage(
											state.copiedPersonalAccessToken
												? desktopBrowserAccessMessages.personalAccessTokenCopied
												: desktopBrowserAccessMessages.copyPersonalAccessToken,
										)}
									</Button>
								</div>
							)}

							<Button
								data-browser-access-action={
									personalAccessTokenConfigured
										? 'reset-personal-access-token'
										: 'create-personal-access-token'
								}
								disabled={busy || state.snapshot.personalAccessToken === 'unavailable'}
								size='sm'
								variant='outline'
								onClick={() => controller.requestPersonalAccessTokenIssue()}
							>
								{intl.formatMessage(personalAccessTokenActionMessage)}
							</Button>
						</section>

						{ready === null ? null : (
							<section className={styles['addresses']} data-slot='desktop-browser-access-addresses'>
								<h3 className={styles['sectionTitle']}>
									{intl.formatMessage(desktopBrowserAccessMessages.addresses)}
								</h3>
								<ul className={styles['addressList']}>
									{addresses.map((address) => (
										<li className={styles['addressRow']} key={address.origin}>
											<div className={styles['addressValue']}>
												<span className={styles['addressLabel']}>
													{intl.formatMessage(address.label)}
												</span>
												<code className={styles['origin']}>{address.origin}</code>
											</div>
											<Button
												disabled={busy}
												size='sm'
												variant='outline'
												onClick={() => controller.copyAccessLink(address.origin)}
											>
												{intl.formatMessage(
													state.copiedOrigin === address.origin
														? desktopBrowserAccessMessages.copied
														: desktopBrowserAccessMessages.copyLink,
												)}
											</Button>
										</li>
									))}
								</ul>
							</section>
						)}

						{ready !== null || runtime.phase === 'stopped' ? (
							<section className={styles['warning']} role='alert'>
								<strong className={styles['warningTitle']}>
									{intl.formatMessage(desktopBrowserAccessMessages.warningTitle)}
								</strong>
								<span>{intl.formatMessage(desktopBrowserAccessMessages.warningDescription)}</span>
							</section>
						) : null}
					</div>

					<DialogFooter className={styles['dialogFooter']}>
						{ready === null ? (
							<>
								<Button
									data-browser-access-action='start-local-network'
									disabled={busy || !personalAccessTokenConfigured}
									variant='outline'
									onClick={() => controller.start('local-network')}
								>
									{intl.formatMessage(desktopBrowserAccessMessages.enableLocalNetwork)}
								</Button>
								<Button
									data-browser-access-action='open-local'
									disabled={busy || !personalAccessTokenConfigured}
									onClick={() => controller.openLocalBrowser()}
								>
									{intl.formatMessage(desktopBrowserAccessMessages.openThisComputer)}
								</Button>
							</>
						) : (
							<>
								<Button
									data-browser-access-action='stop'
									disabled={busy}
									variant='outline'
									onClick={() => controller.stop()}
								>
									{intl.formatMessage(desktopBrowserAccessMessages.stop)}
								</Button>
								<Button
									data-browser-access-action={
										ready.mode === 'loopback' ? 'start-local-network' : 'start-loopback'
									}
									disabled={busy}
									variant='outline'
									onClick={() =>
										controller.start(ready.mode === 'loopback' ? 'local-network' : 'loopback')
									}
								>
									{intl.formatMessage(
										ready.mode === 'loopback'
											? desktopBrowserAccessMessages.enableLocalNetwork
											: desktopBrowserAccessMessages.switchLoopback,
									)}
								</Button>
								<Button
									data-browser-access-action='open-local'
									disabled={busy}
									onClick={() => controller.openLocalBrowser()}
								>
									{intl.formatMessage(desktopBrowserAccessMessages.openBrowser)}
								</Button>
							</>
						)}
					</DialogFooter>
				</DialogContent>
			</Dialog>
			{state.resetConfirmationOpen ? (
				<AlertDialog open>
					<AlertDialogContent
						className={styles['alertDialog']}
						data-slot='desktop-personal-access-token-reset-confirmation'
					>
						<AlertDialogHeader className={styles['alertDialogHeader']}>
							<AlertDialogTitle className={styles['dialogTitle']}>
								{intl.formatMessage(desktopBrowserAccessMessages.personalAccessTokenResetTitle)}
							</AlertDialogTitle>
							<AlertDialogDescription className={styles['dialogDescription']}>
								{intl.formatMessage(
									desktopBrowserAccessMessages.personalAccessTokenResetDescription,
								)}
							</AlertDialogDescription>
						</AlertDialogHeader>
						<AlertDialogFooter className={styles['dialogFooter']}>
							<AlertDialogCancel onClick={() => controller.cancelPersonalAccessTokenReset()}>
								{intl.formatMessage(desktopBrowserAccessMessages.personalAccessTokenResetCancel)}
							</AlertDialogCancel>
							<AlertDialogAction
								data-browser-access-action='confirm-reset-personal-access-token'
								variant='destructive'
								onClick={() => controller.confirmPersonalAccessTokenReset()}
							>
								{intl.formatMessage(desktopBrowserAccessMessages.personalAccessTokenResetConfirm)}
							</AlertDialogAction>
						</AlertDialogFooter>
					</AlertDialogContent>
				</AlertDialog>
			) : null}
		</>
	);
}
