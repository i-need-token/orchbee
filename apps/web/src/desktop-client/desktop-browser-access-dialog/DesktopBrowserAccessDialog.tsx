import {
	desktopBrowserAccessMessages,
	type MessageDescriptor,
	useOrchbeeIntl,
} from '@orchbee/i18n';
import {
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
}

export function DesktopBrowserAccessDialog({
	controller,
}: DesktopBrowserAccessDialogProps): ReactNode {
	const intl = useOrchbeeIntl();
	const state = useDesktopBrowserAccessDialog(controller);
	const busy = state.pendingAction !== null;
	const ready = state.runtime.phase === 'ready' ? state.runtime : null;
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
	if (state.pendingAction?.startsWith('start-') === true || state.runtime.phase === 'starting') {
		statusMessage = desktopBrowserAccessMessages.startProgress;
	} else if (state.pendingAction === 'stop' || state.runtime.phase === 'stopping') {
		statusMessage = desktopBrowserAccessMessages.stopProgress;
	} else if (state.runtime.phase === 'ready') {
		statusMessage =
			state.runtime.mode === 'loopback'
				? desktopBrowserAccessMessages.loopbackStatus
				: desktopBrowserAccessMessages.localNetworkStatus;
	}
	let statusDetail: ReactNode = null;
	if (state.runtime.phase === 'stopped' && state.pendingAction === null) {
		statusDetail = (
			<span className={styles['description']}>
				{intl.formatMessage(desktopBrowserAccessMessages.stoppedDescription)}
			</span>
		);
	}

	return (
		<Dialog
			open={state.open}
			onOpenChange={(open) => {
				if (!open) controller.close();
			}}
		>
			<DialogContent
				closeLabel={intl.formatMessage(desktopBrowserAccessMessages.close)}
				data-slot='desktop-browser-access-dialog'
			>
				<DialogHeader>
					<DialogTitle>{intl.formatMessage(desktopBrowserAccessMessages.title)}</DialogTitle>
					<DialogDescription>
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

					{ready === null ? null : (
						<section className={styles['addresses']}>
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

					{ready !== null || state.runtime.phase === 'stopped' ? (
						<section className={styles['warning']} role='alert'>
							<strong className={styles['warningTitle']}>
								{intl.formatMessage(desktopBrowserAccessMessages.warningTitle)}
							</strong>
							<span>{intl.formatMessage(desktopBrowserAccessMessages.warningDescription)}</span>
						</section>
					) : null}
				</div>

				<DialogFooter>
					{ready === null ? (
						<>
							<Button
								data-browser-access-action='start-local-network'
								disabled={busy}
								variant='outline'
								onClick={() => controller.start('local-network')}
							>
								{intl.formatMessage(desktopBrowserAccessMessages.enableLocalNetwork)}
							</Button>
							<Button
								data-browser-access-action='open-local'
								disabled={busy}
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
	);
}
