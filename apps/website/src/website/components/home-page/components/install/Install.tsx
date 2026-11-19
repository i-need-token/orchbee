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
	offersMacos,
	releaseNotesUrl,
	SOURCE_COMMANDS,
	type Release,
} from '../../../../release';
import { richText } from '../../../../rich-text';
import { CommandChip } from '../../../command-chip/CommandChip';
import { LinkButton } from '../../../link-button/LinkButton';
import { SectionHead } from '../../../section-head/SectionHead';
import styles from './Install.module.css';

type InstallGroup = 'windows' | 'macos' | 'npm' | 'all';

function InstallRow({
	title,
	detail,
	stacked = false,
	group,
	children,
}: {
	title: ReactNode;
	detail?: ReactNode;
	/** Puts the content under the text, for content too wide to sit beside it. */
	stacked?: boolean;
	group?: InstallGroup;
	children: ReactNode;
}): ReactElement {
	return (
		<div className={stacked ? styles['stackedRow'] : styles['row']} data-install-group={group}>
			<div>
				<b className={styles['title']}>{title}</b>
				{detail === undefined ? null : <small className={styles['detail']}>{detail}</small>}
			</div>
			{children}
		</div>
	);
}

// How to install Orchbee. Once a release is published: the Windows installer, the npm package and
// how to start, with the release named; with a macOS download as well, a platform switcher that
// focuses the visitor's OS (Windows, macOS, or npm), showing the script commands and disk images;
// before a release, the source build.
export function Install({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const macos = release.published ? macosDownloads(release.downloads) : [];
	const withMacos = offersMacos(release);
	return (
		<section className={styles['section']} id='install' data-install=''>
			<div className={styles['install']} data-reveal=''>
				<div>
					<SectionHead
						layout='stack'
						kicker={t(messages.installKicker)}
						title={t(messages.installTitle)}
						lead={t(withMacos ? messages.installLeadMacos : messages.installLead)}
					/>
					{release.published ? (
						<a
							className={styles['release']}
							href={releaseNotesUrl(release.version)}
							target='_blank'
							rel='noreferrer'
						>
							{t(messages.installReleaseNotes, { version: release.version })} ↗
						</a>
					) : null}
				</div>
				<div>
					{release.published ? (
						<>
							{withMacos ? (
								<div
									className={styles['tabs']}
									role='group'
									aria-label={t(messages.installPlatform)}
									data-install-tabs=''
									hidden
								>
									<button
										className={styles['tab']}
										type='button'
										data-install-tab='windows'
										aria-pressed='true'
									>
										{t(messages.installTabWindows)}
									</button>
									<button
										className={styles['tab']}
										type='button'
										data-install-tab='macos'
										aria-pressed='false'
									>
										{t(messages.installTabMacos)}
									</button>
									<button
										className={styles['tab']}
										type='button'
										data-install-tab='npm'
										aria-pressed='false'
									>
										{t(messages.installTabNpm)}
									</button>
								</div>
							) : null}
							<InstallRow
								title={t(withMacos ? messages.installAppWindows : messages.installApp)}
								detail={t(shellMessages.installerNote, richText)}
								group='windows'
							>
								<LinkButton href={release.downloads['windows-x86_64']} icon='download'>
									{t(shellMessages.downloadForWindows)}
								</LinkButton>
							</InstallRow>
							{withMacos ? (
								<>
									<InstallRow
										title={t(messages.installMacosScript)}
										detail={t(messages.installMacosScriptNote)}
										stacked
										group='macos'
									>
										<div className={styles['commands']}>
											<div className={styles['commandItem']}>
												<span className={styles['commandLabel']}>
													{t(messages.installMacosScriptDesktop)}
												</span>
												<CommandChip command={INSTALL_SCRIPT_DESKTOP_COMMAND} tone='quiet' />
											</div>
											<div className={styles['commandItem']}>
												<span className={styles['commandLabel']}>
													{t(messages.installMacosScriptCli)}
												</span>
												<CommandChip command={INSTALL_SCRIPT_COMMAND} tone='quiet' />
											</div>
										</div>
									</InstallRow>
									<InstallRow
										title={t(messages.installMacosApp)}
										detail={t(messages.installMacosAppNote)}
										group='macos'
									>
										<div className={styles['downloads']}>
											{macos.map(({ platform, url }, index) => (
												<LinkButton href={url} icon='download' primary={index === 0} key={platform}>
													{t(
														platform === 'macos-aarch64'
															? messages.installDownloadAppleSilicon
															: messages.installDownloadIntel,
													)}
												</LinkButton>
											))}
										</div>
									</InstallRow>
								</>
							) : null}
							<InstallRow
								title={t(messages.installNpm)}
								detail={t(messages.installNpmNote)}
								group='npm'
							>
								<CommandChip command={NPM_INSTALL_COMMAND} tone='quiet' />
							</InstallRow>
							<InstallRow
								title={t(messages.installStart)}
								detail={t(withMacos ? messages.installStartNoteMacos : messages.installStartNote)}
								group='all'
							>
								<CommandChip command='orchbee' tone='quiet' />
							</InstallRow>
							<p className={styles['note']} data-install-note='windows'>
								{t(messages.installNote)}
							</p>
							{withMacos ? (
								<p className={styles['note']} data-install-note='macos'>
									{t(messages.installMacosNote, richText)}
								</p>
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
