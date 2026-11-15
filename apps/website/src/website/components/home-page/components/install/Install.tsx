import {
	useOrchbeeIntl,
	websiteHomeMessages as messages,
	websiteShellMessages as shellMessages,
} from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import {
	INSTALL_SCRIPT_COMMAND,
	INSTALL_SCRIPT_DESKTOP_COMMAND,
	macosDownloads,
	NPM_INSTALL_COMMAND,
	releaseNotesUrl,
	SOURCE_COMMANDS,
	type Release,
} from '../../../../release';
import { richText } from '../../../../rich-text';
import { CommandChip } from '../../../command-chip/CommandChip';
import { LinkButton } from '../../../link-button/LinkButton';
import { SectionHead } from '../../../section-head/SectionHead';
import styles from './Install.module.css';

function InstallRow({
	title,
	detail,
	stacked = false,
	children,
}: {
	title: ReactNode;
	detail?: ReactNode;
	/** Puts the content under the text, for content too wide to sit beside it. */
	stacked?: boolean;
	children: ReactNode;
}): ReactElement {
	return (
		<div className={stacked ? styles['stackedRow'] : styles['row']}>
			<div>
				<b className={styles['title']}>{title}</b>
				{detail === undefined ? null : <small className={styles['detail']}>{detail}</small>}
			</div>
			{children}
		</div>
	);
}

// How to install Orchbee. Once a release is published: the Windows installer, the npm package and
// how to start, with the release named; with a macOS download as well, the disk images, the
// install script and the Open Anyway steps; before a release, the source build.
export function Install({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const macos = release.published ? macosDownloads(release.downloads) : [];
	const offersMacos = macos.length > 0;
	return (
		<section className={styles['section']} id='install'>
			<div className={styles['install']} data-reveal=''>
				<div>
					<SectionHead
						layout='stack'
						kicker={t(messages.installKicker)}
						title={t(messages.installTitle)}
						lead={t(offersMacos ? messages.installLeadMacos : messages.installLead)}
					/>
					{release.published ? (
						<a
							className={styles['release']}
							href={releaseNotesUrl(release.version)}
							target='_blank'
							rel='noreferrer'
						>
							{t(messages.installReleaseNotes, { version: release.version })}
						</a>
					) : null}
				</div>
				<div>
					{release.published ? (
						<>
							<InstallRow
								title={t(offersMacos ? messages.installAppWindows : messages.installApp)}
								detail={t(shellMessages.installerNote, richText)}
							>
								<LinkButton href={release.downloads['windows-x86_64']} icon='download'>
									{t(shellMessages.downloadForWindows)}
								</LinkButton>
							</InstallRow>
							{offersMacos ? (
								<>
									<InstallRow
										title={t(messages.installMacosApp)}
										detail={t(messages.installMacosAppNote)}
									>
										<div className={styles['downloads']}>
											{macos.map(({ platform, url }) => (
												<LinkButton href={url} icon='download' key={platform}>
													{t(
														platform === 'macos-aarch64'
															? messages.installDownloadAppleSilicon
															: messages.installDownloadIntel,
													)}
												</LinkButton>
											))}
										</div>
									</InstallRow>
									<InstallRow
										title={t(messages.installMacosScript)}
										detail={t(messages.installMacosScriptNote)}
										stacked
									>
										<div className={styles['commands']}>
											<CommandChip command={INSTALL_SCRIPT_COMMAND} tone='quiet' />
											<CommandChip command={INSTALL_SCRIPT_DESKTOP_COMMAND} tone='quiet' />
										</div>
									</InstallRow>
								</>
							) : null}
							<InstallRow title={t(messages.installNpm)} detail={t(messages.installNpmNote)}>
								<CommandChip command={NPM_INSTALL_COMMAND} tone='quiet' />
							</InstallRow>
							<InstallRow
								title={t(messages.installStart)}
								detail={t(offersMacos ? messages.installStartNoteMacos : messages.installStartNote)}
							>
								<CommandChip command='orchbee' tone='quiet' />
							</InstallRow>
							<p className={styles['note']}>{t(messages.installNote)}</p>
							{offersMacos ? (
								<p className={styles['note']}>{t(messages.installMacosNote, richText)}</p>
							) : null}
						</>
					) : (
						<>
							<InstallRow
								title={t(messages.installSource)}
								detail={t(messages.installSourceRequirements)}
							>
								<div className={styles['commands']}>
									{SOURCE_COMMANDS.map((command) => (
										<CommandChip command={command} tone='quiet' key={command} />
									))}
								</div>
							</InstallRow>
							<InstallRow title={t(messages.installSourceStart)}>
								<CommandChip command='pnpm desktop' tone='quiet' />
							</InstallRow>
							<p className={styles['note']}>{t(messages.installSourceNote)}</p>
						</>
					)}
				</div>
			</div>
		</section>
	);
}
