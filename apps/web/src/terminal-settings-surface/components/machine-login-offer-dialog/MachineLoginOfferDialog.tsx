import {
	Button,
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	Kbd,
} from '@orchbee/ui/components';
import * as React from 'react';

import { useTerminalSettingsMachines } from '../../hooks/useTerminalSettingsMachines';
import type {
	TerminalSettingsMachines,
	TerminalSettingsMachinesView,
} from '../../types/terminal-settings-surface';
import styles from './MachineLoginOfferDialog.module.css';

type Offer = NonNullable<TerminalSettingsMachinesView['offer']>;

/** How long the notice shows, with no key pressed on it, before a choice counts. */
const ARMING_MILLISECONDS = 1000;

interface MachineLoginOfferDialogProps {
	readonly machines: TerminalSettingsMachines;
}

/**
 * The notice of a command login the hub offers this window, as the TUI's notice: which machine,
 * how many commands wait, and whether to log in now; Log in claims the login, whose question then
 * asks in the login Dialog, and Not now fails the commands. It opens over whatever holds the
 * window, and a choice counts only once it has shown for a second with no key pressed on it, so an
 * Enter meant for a terminal never logs in or fails a command.
 */
export function MachineLoginOfferDialog({
	machines,
}: MachineLoginOfferDialogProps): React.ReactNode {
	const offer = useTerminalSettingsMachines(machines)?.offer ?? null;
	if (offer === null) return null;
	return <MachineLoginOffer key={offer.login} machines={machines} offer={offer} />;
}

function MachineLoginOffer({
	machines,
	offer,
}: {
	readonly machines: TerminalSettingsMachines;
	readonly offer: Offer;
}): React.ReactNode {
	const armed = useArmed(offer.ended === null ? 'open' : 'ended');
	// Enter's button holds the focus first, so the ring and the key name the same choice.
	const primary = React.useRef<HTMLButtonElement>(null);
	const ended = offer.ended !== null;
	const choosable = armed.now && (ended || !offer.accepted);
	const choose = (accept: boolean): void => {
		if (!choosable) {
			armed.restart();
			return;
		}
		machines.request({ act: accept ? 'offer-accept' : 'offer-decline', login: offer.login });
	};
	return (
		<Dialog
			disablePointerDismissal
			open
			onOpenChange={(open) => {
				if (!open) choose(false);
			}}
		>
			<DialogContent
				data-slot='machine-login-offer-dialog'
				initialFocus={primary}
				showCloseButton={false}
				onKeyDownCapture={(event) => {
					// Every key but an armed Enter or Escape starts the second again; Enter on a button
					// presses that button.
					if (event.key === 'Enter') {
						if (event.target instanceof HTMLButtonElement) return;
						event.preventDefault();
						choose(!ended);
					} else if (event.key !== 'Escape') {
						armed.restart();
					}
				}}
			>
				<DialogHeader className={styles['header']}>
					<DialogTitle className={styles['title']}>{offer.labels.title}</DialogTitle>
				</DialogHeader>
				<div aria-live='polite' className={styles['body']}>
					<p className={styles['machine']}>{offer.machine}</p>
					{offer.ended === null ? (
						<>
							{offer.count === null ? null : <p className={styles['count']}>{offer.count}</p>}
							{offer.consequence === null ? null : (
								<p className={styles['consequence']}>{offer.consequence}</p>
							)}
							{offer.accepted ? <p className={styles['pending']}>{offer.labels.starting}</p> : null}
						</>
					) : (
						<p className={styles[offer.ended.tone]}>{offer.ended.text}</p>
					)}
				</div>
				<DialogFooter>
					{ended ? (
						<Button
							ref={primary}
							className={styles['action']}
							type='button'
							onClick={() => choose(false)}
						>
							{offer.labels.close}
							<Kbd aria-hidden='true' className={styles['shortcut']}>
								Enter
							</Kbd>
						</Button>
					) : (
						<>
							<Button
								className={styles['action']}
								disabled={offer.accepted}
								type='button'
								variant='outline'
								onClick={() => choose(false)}
							>
								{offer.labels.decline}
								<Kbd aria-hidden='true' className={styles['shortcut']}>
									Esc
								</Kbd>
							</Button>
							<Button
								ref={primary}
								className={styles['action']}
								disabled={offer.accepted}
								type='button'
								onClick={() => choose(true)}
							>
								{offer.labels.logIn}
								<Kbd aria-hidden='true' className={styles['shortcut']}>
									Enter
								</Kbd>
							</Button>
						</>
					)}
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}

/**
 * Whether the notice has shown for a second since `phase` began, the window last came into view,
 * or `restart` last ran.
 */
function useArmed(phase: string): { readonly now: boolean; readonly restart: () => void } {
	const [round, setRound] = React.useState(0);
	const [armedAt, setArmedAt] = React.useState<string | null>(null);
	const key = `${phase}:${round}`;
	React.useEffect(() => {
		const timer = window.setTimeout(() => setArmedAt(key), ARMING_MILLISECONDS);
		return () => window.clearTimeout(timer);
	}, [key]);
	React.useEffect(() => {
		const shown = (): void => {
			if (document.visibilityState === 'visible') setRound((value) => value + 1);
		};
		document.addEventListener('visibilitychange', shown);
		return () => document.removeEventListener('visibilitychange', shown);
	}, []);
	return { now: armedAt === key, restart: () => setRound((value) => value + 1) };
}
