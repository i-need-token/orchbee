import {
	terminalSettingsMessages,
	useOrchbeeIntl,
	workbenchMessages,
	type MessageDescriptor,
} from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import * as React from 'react';

import type {
	TerminalSettingsAbout,
	TerminalSettingsUpdateCheckStatus,
	TerminalSettingsUpdateOffer,
} from '../../../../types/terminal-settings-surface';
import { SettingFailure } from '../../../setting-failure/SettingFailure';
import { SettingsGroup } from '../../../settings-group/SettingsGroup';
import { SettingsRow } from '../../../settings-row/SettingsRow';
import styles from './AboutUpdateStatus.module.css';

interface AboutUpdateStatusProps {
	readonly about: TerminalSettingsAbout;
	/** Rows the card holds under Updates. */
	readonly children: React.ReactNode;
}

const BUILD_LABELS = {
	development: terminalSettingsMessages.pageAboutBuildDevelopment,
	release: terminalSettingsMessages.pageAboutBuildRelease,
	test: terminalSettingsMessages.pageAboutBuildTest,
} as const satisfies Readonly<
	Record<NonNullable<TerminalSettingsAbout['build']>, MessageDescriptor>
>;

const UNAVAILABLE_DESCRIPTIONS = {
	development: terminalSettingsMessages.pageAboutUpdatesUnavailableDevelopment,
	installation: terminalSettingsMessages.pageAboutUpdatesUnavailableInstallation,
	session: terminalSettingsMessages.pageAboutUpdatesUnavailableSession,
} as const satisfies Readonly<Record<string, MessageDescriptor>>;

/** Why a Session move waits, as the window's notice words it. */
const SESSION_WAITS = {
	'background-programs': workbenchMessages.updateSessionBackgroundPrograms,
	clients: workbenchMessages.updateSessionClients,
	'host-required': workbenchMessages.updateSessionHostRequired,
	other: workbenchMessages.updateSessionOther,
} as const satisfies Readonly<Record<string, MessageDescriptor>>;

const NO_INSTALLATION: TerminalSettingsUpdateCheckStatus = {
	kind: 'unavailable',
	reason: 'installation',
};
const readNoInstallation = (): TerminalSettingsUpdateCheckStatus => NO_INSTALLATION;
const readNoOffer = (): null => null;
const subscribeToNothing = (): (() => void) => () => undefined;

/**
 * The version this window runs and its build, and Updates, whose control is Check for updates until
 * a newer version or a move of an older Session is offered and Update after, except while that move
 * runs; the card's remaining rows follow Updates.
 */
export function AboutUpdateStatus({ about, children }: AboutUpdateStatusProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const { check, offer: offerSource } = about;
	const offer = React.useSyncExternalStore(
		offerSource?.subscribe ?? subscribeToNothing,
		offerSource?.getSnapshot ?? readNoOffer,
		offerSource?.getSnapshot ?? readNoOffer,
	);
	const status = React.useSyncExternalStore(
		check?.subscribe ?? subscribeToNothing,
		check?.getSnapshot ?? readNoInstallation,
		check?.getSnapshot ?? readNoInstallation,
	);
	// Showing About clears a finished result and reads where the Session's job stands.
	React.useEffect(() => check?.show(), [check]);
	const [offerFailure, setOfferFailure] = React.useState(() => ({
		count: 0,
		source: offerSource,
	}));
	if (offerFailure.source !== offerSource) {
		setOfferFailure({ count: 0, source: offerSource });
	}
	React.useEffect(
		() =>
			offerSource?.subscribeFailed?.(() =>
				setOfferFailure((current) => ({ count: current.count + 1, source: offerSource })),
			),
		[offerSource],
	);
	const failureCount =
		(offerFailure.source === offerSource ? offerFailure.count : 0) +
		(about.updateFailureCount ?? 0);

	const updatesLabel = intl.formatMessage(terminalSettingsMessages.pageAboutUpdatesLabel);
	let description: string;
	let tone: 'danger' | 'positive' | undefined;
	let control: React.ReactNode;
	if (offer !== null) {
		description = offerDescription(intl, offer);
		control =
			offer.kind === 'session' && offer.state === 'updating' ? null : (
				<Button data-about-update size='sm' type='button' onClick={about.update}>
					{intl.formatMessage(workbenchMessages.update)}
				</Button>
			);
	} else {
		description = intl.formatMessage(updatesDescription(status));
		tone = statusTone(status);
		const checking = status.kind === 'checking';
		const installerOffered = status.kind === 'failed' || isSessionUnavailable(status);
		control = (
			<div className={styles['actions']}>
				{installerOffered && about.downloadInstaller !== undefined ? (
					<Button
						data-about-download
						size='sm'
						type='button'
						variant='outline'
						onClick={about.downloadInstaller}
					>
						{intl.formatMessage(terminalSettingsMessages.pageAboutUpdatesDownload)}
					</Button>
				) : null}
				<Button
					data-about-check
					disabled={checking || status.kind === 'unavailable' || check === undefined}
					size='sm'
					type='button'
					variant='outline'
					onClick={check?.start}
				>
					{intl.formatMessage(
						checking
							? terminalSettingsMessages.pageAboutUpdatesChecking
							: terminalSettingsMessages.pageAboutUpdatesCheck,
					)}
				</Button>
			</div>
		);
	}
	return (
		<>
			{failureCount > 0 ? (
				<SettingFailure
					key={failureCount}
					message={intl.formatMessage(workbenchMessages.updateUnavailable)}
				/>
			) : null}
			<SettingsGroup>
				{about.version === null ? null : (
					<SettingsRow
						description={
							about.build === null ? null : intl.formatMessage(BUILD_LABELS[about.build])
						}
						label={intl.formatMessage(terminalSettingsMessages.pageAboutVersionLabel)}
						query=''
					>
						<span className={styles['value']} data-about-version>
							{about.version}
						</span>
					</SettingsRow>
				)}
				<SettingsRow
					description={description}
					descriptionTone={tone}
					label={updatesLabel}
					query=''
					status={
						<span aria-live='polite' className={styles['announcement']} role='status'>
							{offer === null && tone !== undefined ? description : ''}
						</span>
					}
				>
					{control}
				</SettingsRow>
				{children}
			</SettingsGroup>
		</>
	);
}

function updatesDescription(status: TerminalSettingsUpdateCheckStatus): MessageDescriptor {
	switch (status.kind) {
		case 'idle':
		case 'checking':
			return terminalSettingsMessages.pageAboutUpdatesSchedule;
		case 'up-to-date':
			return terminalSettingsMessages.pageAboutUpdatesUpToDate;
		case 'failed':
			return terminalSettingsMessages.pageAboutUpdatesFailed;
		case 'unavailable':
			return UNAVAILABLE_DESCRIPTIONS[status.reason];
	}
}

/** A Session too old to run checks, like a failed check, leaves installing from orchbee.com open. */
function isSessionUnavailable(status: TerminalSettingsUpdateCheckStatus): boolean {
	return status.kind === 'unavailable' && status.reason === 'session';
}

function statusTone(status: TerminalSettingsUpdateCheckStatus): 'danger' | 'positive' | undefined {
	if (status.kind === 'up-to-date') return 'positive';
	if (status.kind === 'failed') return 'danger';
	return undefined;
}

/** What Updates says while Update is offered: the switch, or the Session move and why it waits. */
function offerDescription(
	intl: ReturnType<typeof useOrchbeeIntl>,
	offer: TerminalSettingsUpdateOffer,
): string {
	if (offer.kind === 'client') {
		return intl.formatMessage(workbenchMessages.updateTooltip, { version: offer.version });
	}
	if (offer.state === 'updating') {
		return intl.formatMessage(workbenchMessages.updateSessionUpdating, { version: offer.version });
	}
	return intl.formatMessage(SESSION_WAITS[offer.state], { sessionVersion: offer.sessionVersion });
}
