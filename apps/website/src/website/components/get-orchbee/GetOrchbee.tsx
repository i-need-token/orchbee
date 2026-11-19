import {
	useOrchbeeIntl,
	websiteShellMessages as messages,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { pagePath } from '../../pages';
import { BUILD_GUIDE_URL, NPM_INSTALL_COMMAND, offersMacos, type Release } from '../../release';
import { CommandChip } from '../command-chip/CommandChip';
import { LinkButton } from '../link-button/LinkButton';
import styles from './GetOrchbee.module.css';

// The way to get Orchbee that a call to action offers. Once a release is published it offers direct
// platform downloads (with the visitor's OS highlighted as primary by activateGetOrchbee) and a
// direct link to all installation options, or the command line package. Before a release, every
// call to action leads to the source build.
export function GetOrchbee({
	locale,
	release,
	offer,
}: {
	locale: OrchbeeLocale;
	release: Release;
	offer: 'both' | 'installer' | 'npm';
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	if (!release.published) {
		return (
			<div className={styles['root']}>
				<div className={styles['actions']}>
					<LinkButton href={BUILD_GUIDE_URL}>{t(messages.getSource)}</LinkButton>
				</div>
			</div>
		);
	}
	const withMacos = offersMacos(release);
	const macPrimaryUrl = release.downloads['macos-aarch64'] ?? release.downloads['macos-x86_64'];
	const macIntelUrl =
		release.downloads['macos-aarch64'] !== undefined
			? release.downloads['macos-x86_64']
			: undefined;
	const installHref = `${pagePath('home', locale)}#install`;

	return (
		<div className={styles['root']} data-get-orchbee=''>
			<div className={styles['actions']}>
				{offer === 'npm' ? (
					<CommandChip command={NPM_INSTALL_COMMAND} />
				) : (
					<>
						<LinkButton
							href={release.downloads['windows-x86_64']}
							icon='download'
							platform='windows'
							primary
						>
							{t(messages.downloadForWindows)}
						</LinkButton>
						{withMacos && macPrimaryUrl !== undefined ? (
							<LinkButton href={macPrimaryUrl} icon='download' platform='macos' primary={false}>
								{t(
									release.downloads['macos-aarch64'] !== undefined
										? messages.downloadForMacAppleSilicon
										: messages.downloadForMac,
								)}
							</LinkButton>
						) : null}
					</>
				)}
			</div>
			<div className={styles['meta']}>
				{offer !== 'npm' && macIntelUrl !== undefined ? (
					<>
						<a className={styles['link']} href={macIntelUrl}>
							↓ {t(messages.downloadForMacIntel)} (.dmg)
						</a>
						<span className={styles['dot']} aria-hidden='true'>
							·
						</span>
					</>
				) : null}
				<a className={styles['link']} href={installHref}>
					{t(messages.allInstallOptions)} (curl / npm) →
				</a>
			</div>
		</div>
	);
}
