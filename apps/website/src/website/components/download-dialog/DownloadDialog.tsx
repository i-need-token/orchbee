import { useOrchbeeIntl, websiteShellMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import {
	INSTALL_SCRIPT_COMMAND,
	INSTALL_SCRIPT_DESKTOP_COMMAND,
	macosDownloads,
	NPM_INSTALL_COMMAND,
	offersMacos,
	releaseNotesUrl,
	type Release,
} from '../../release';
import { CommandChip } from '../command-chip/CommandChip';
import { Icon } from '../icon/Icon';
import { LinkButton } from '../link-button/LinkButton';
import styles from './DownloadDialog.module.css';

// The dialog that the navigation bar and the home hero open with their Download buttons: each installer
// of the release, then the other ways to install: the npm package and, once a macOS disk image is
// published, the install script. It is rendered only after a release is published, so nothing unshipped
// appears before one. activateDownloadDialog lists the visitor's platform first and opens the dialog for
// #download.
export function DownloadDialog({ release }: { release: Release }): ReactElement | null {
	const { formatMessage: t } = useOrchbeeIntl();
	if (!release.published) return null;
	const downloads = [
		{
			platform: 'windows' as const,
			key: 'windows-x86_64',
			url: release.downloads['windows-x86_64'],
			label: messages.downloadForWindows,
		},
		...macosDownloads(release.downloads).map(({ platform, url }) => ({
			platform: 'macos' as const,
			key: platform,
			url,
			label:
				platform === 'macos-aarch64'
					? messages.downloadForMacAppleSilicon
					: messages.downloadMacIntel,
		})),
	];
	const methods = [
		{ key: 'npm', label: messages.downloadNpm, command: NPM_INSTALL_COMMAND },
		...(offersMacos(release)
			? [
					{
						key: 'script-desktop',
						label: messages.downloadScriptDesktop,
						command: INSTALL_SCRIPT_DESKTOP_COMMAND,
					},
					{
						key: 'script-cli',
						label: messages.downloadScriptCli,
						command: INSTALL_SCRIPT_COMMAND,
					},
				]
			: []),
	];
	return (
		<div className={styles['backdrop']} data-download-dialog='' hidden>
			<div
				className={styles['panel']}
				role='dialog'
				aria-modal='true'
				aria-labelledby='download-dialog-title'
				data-download-panel=''
			>
				<div className={styles['head']}>
					<div className={styles['titleRow']}>
						<h2 className={styles['title']} id='download-dialog-title'>
							{t(messages.downloadTitle)}
						</h2>
						<button
							className={styles['close']}
							type='button'
							aria-label={t(messages.downloadClose)}
							data-download-close=''
						>
							<Icon name='close' />
						</button>
					</div>
					<a
						className={styles['notes']}
						href={releaseNotesUrl(release.version)}
						target='_blank'
						rel='noreferrer'
					>
						{t(messages.downloadReleaseNotes, { version: release.version })} ↗
					</a>
				</div>
				<div className={styles['downloads']} data-download-list=''>
					{downloads.map(({ platform, key, url, label }, index) => (
						<div className={styles['download']} data-download-item={platform} key={key}>
							<LinkButton href={url} icon='download' primary={index === 0}>
								{t(label)}
							</LinkButton>
						</div>
					))}
				</div>
				<section className={styles['other']} aria-labelledby='download-dialog-other'>
					<h3 className={styles['kicker']} id='download-dialog-other'>
						{t(messages.downloadOther)}
					</h3>
					<div className={styles['methods']}>
						{methods.map(({ key, label, command }) => (
							<div className={styles['method']} key={key}>
								<span className={styles['label']}>{t(label)}</span>
								<CommandChip command={command} tone='quiet' />
							</div>
						))}
					</div>
				</section>
			</div>
		</div>
	);
}
