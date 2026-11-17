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
} from '../../types/terminal-settings-surface';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';
import styles from './AboutSettings.module.css';

interface AboutSettingsProps {
	readonly about: TerminalSettingsAbout;
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

const NO_INSTALLATION: TerminalSettingsUpdateCheckStatus = {
	kind: 'unavailable',
	reason: 'installation',
};
const readNoInstallation = (): TerminalSettingsUpdateCheckStatus => NO_INSTALLATION;
const readNoOffer = (): null => null;
const subscribeToNothing = (): (() => void) => () => undefined;

/**
 * About: the version this window runs and its build, and Updates, whose control is Check for
 * updates until a newer version is offered and Update after. It holds no setting, so it has no
 * settings-file line and search never lists it.
 */
export function AboutSettings({ about }: AboutSettingsProps): React.ReactNode {
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

	const updatesLabel = intl.formatMessage(terminalSettingsMessages.pageAboutUpdatesLabel);
	let description: string;
	let tone: 'danger' | 'positive' | undefined;
	let control: React.ReactNode;
	if (offer !== null) {
		description = intl.formatMessage(workbenchMessages.updateTooltip, { version: offer });
		control = (
			<Button data-about-update size='sm' type='button' onClick={about.update}>
				{intl.formatMessage(workbenchMessages.update)}
			</Button>
		);
	} else {
		description = intl.formatMessage(updatesDescription(status));
		tone = statusTone(status);
		const checking = status.kind === 'checking';
		control = (
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
		);
	}
	return (
		<SettingsGroup>
			{about.version === null ? null : (
				<SettingsRow
					description={about.build === null ? null : intl.formatMessage(BUILD_LABELS[about.build])}
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
		</SettingsGroup>
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

function statusTone(status: TerminalSettingsUpdateCheckStatus): 'danger' | 'positive' | undefined {
	if (status.kind === 'up-to-date') return 'positive';
	if (status.kind === 'failed') return 'danger';
	return undefined;
}
