import { useOrchbeeIntl, websiteShellMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import {
	NPM_INSTALL_COMMAND,
	publishedPlatforms,
	releaseNotesUrl,
	type Release,
} from '../../release';
import { CommandChip } from '../command-chip/CommandChip';
import { Icon } from '../icon/Icon';
import { LinkButton } from '../link-button/LinkButton';
import styles from './DownloadDialog.module.css';

// The dialog that the navigation bar and the home hero open with their Download buttons: the installers of the
// release grouped by platform, each group under the platform's icon and name, then the other ways to install:
// the install commands of each platform the release published, the desktop application's first and a command
// several of them offer listed once under its shared label, then the npm package, which has no desktop
// application. It is rendered only after a release is published, so nothing unshipped appears before one.
// activateDownloadDialog lists the visitor's platform first and opens the dialog for #download.
export function DownloadDialog({ release }: { release: Release }): ReactElement | null {
	const { formatMessage: t } = useOrchbeeIntl();
	if (!release.published) return null;
	const platforms = publishedPlatforms(release);
	const commands = platforms.flatMap(({ platform }) => platform.commands);
	const methods = [
		...commands.flatMap(({ label, sharedLabel, command }, index) => {
			if (commands.findIndex((other) => other.command === command) !== index) return [];
			const shared = commands.some((other, at) => at !== index && other.command === command);
			return [
				{
					key: command,
					label: messages[shared && sharedLabel !== undefined ? sharedLabel : label],
					command,
				},
			];
		}),
		{ key: 'npm', label: messages.downloadNpm, command: NPM_INSTALL_COMMAND },
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
					<p className={styles['handheld']} data-download-handheld='' hidden>
						{t(messages.downloadHandheld)}
					</p>
				</div>
				<div className={styles['downloads']} data-download-list=''>
					{platforms.map(({ platform, variants }, groupIndex) => {
						const headingId = `download-group-${platform.id}`;
						return (
							<section
								className={styles['group']}
								data-download-group={platform.id}
								aria-labelledby={headingId}
								key={platform.id}
							>
								<h3 className={styles['groupTitle']} id={headingId}>
									<Icon name={platform.icon} />
									<span className={styles['groupName']}>{t(messages[platform.name])}</span>
								</h3>
								<div className={styles['variants']}>
									{variants.map(({ download, label, url }, variantIndex) => (
										<LinkButton
											href={url}
											icon='download'
											primary={groupIndex === 0 && variantIndex === 0}
											key={download}
										>
											{t(messages[label])}
										</LinkButton>
									))}
								</div>
							</section>
						);
					})}
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
