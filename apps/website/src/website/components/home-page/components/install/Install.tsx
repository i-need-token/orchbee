import {
	useOrchbeeIntl,
	websiteHomeMessages as messages,
	websiteShellMessages as shellMessages,
} from '@orchbee/i18n';
import { Fragment, type ReactElement, type ReactNode } from 'react';

import {
	NPM_INSTALL_COMMAND,
	offeredPlatforms,
	type OfferedPlatforms,
	publishedPlatforms,
	releaseNotesUrl,
	SOURCE_COMMANDS,
	type Release,
} from '../../../../release';
import { richText } from '../../../../rich-text';
import { CommandChip } from '../../../command-chip/CommandChip';
import { Icon } from '../../../icon/Icon';
import { LinkButton } from '../../../link-button/LinkButton';
import { SectionHead } from '../../../section-head/SectionHead';
import styles from './Install.module.css';

// `group` names the tab a row belongs to: a platform's id, `npm`, or `all` for a row that every tab shows.
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
	group?: string;
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

// The install lead and the start note, by the platforms besides Windows the release offers.
const LEAD = {
	windows: messages.installLead,
	macos: messages.installLeadMacos,
	linux: messages.installLeadLinux,
	all: messages.installLeadAll,
} as const satisfies Record<OfferedPlatforms, unknown>;
const START_NOTE = {
	windows: messages.installStartNote,
	macos: messages.installStartNoteMacos,
	linux: messages.installStartNoteLinux,
	all: messages.installStartNoteAll,
} as const satisfies Record<OfferedPlatforms, unknown>;

// How to install Orchbee. Once a release is published: the rows of each platform it published (its installer buttons,
// then its commands when it has any), the npm package and how to start, with the release named. With more than one
// platform, a tab per platform and one for npm, opening on the visitor's platform; the note under the rows follows
// the tab. Before a release, the source build.
export function Install({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const platforms = publishedPlatforms(release);
	const several = platforms.length > 1;
	const offered = offeredPlatforms(release);
	return (
		<section className={styles['section']} id='install' data-install=''>
			<div className={styles['install']} data-reveal=''>
				<div>
					<SectionHead
						layout='stack'
						kicker={t(messages.installKicker)}
						title={t(messages.installTitle)}
						lead={t(LEAD[offered])}
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
							{several ? (
								<div
									className={styles['tabs']}
									role='group'
									aria-label={t(messages.installPlatform)}
									data-install-tabs=''
									hidden
								>
									{platforms.map(({ platform }, index) => (
										<button
											className={styles['tab']}
											type='button'
											data-install-tab={platform.id}
											aria-pressed={index === 0 ? 'true' : 'false'}
											key={platform.id}
										>
											<Icon name={platform.icon} />
											{t(shellMessages[platform.name])}
										</button>
									))}
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
							{platforms.map(({ platform, variants }) => {
								const installers = variants.map(({ download, install, url }, variantIndex) => (
									<LinkButton
										href={url}
										icon='download'
										primary={variantIndex === 0}
										key={download}
									>
										{t(shellMessages[install])}
									</LinkButton>
								));
								return (
									<Fragment key={platform.id}>
										<InstallRow
											title={
												several ? t(shellMessages[platform.install.title]) : t(messages.installApp)
											}
											detail={t(shellMessages[platform.install.detail], richText)}
											// More than two installers, as Linux has, leave the text no room beside them.
											stacked={installers.length > 2}
											group={platform.id}
										>
											{/* A lone installer sits in the row itself, as the Windows download always has. */}
											{installers.length === 1 ? (
												installers
											) : (
												<div className={styles['downloads']}>{installers}</div>
											)}
										</InstallRow>
										{platform.install.commands === undefined ? null : (
											<InstallRow
												title={t(shellMessages[platform.install.commands.title])}
												detail={t(shellMessages[platform.install.commands.detail])}
												stacked
												group={platform.id}
											>
												<div className={styles['commands']}>
													{platform.commands.map((command) => (
														<div className={styles['commandItem']} key={command.command}>
															<span className={styles['commandLabel']}>
																{t(shellMessages[command.install])}
															</span>
															<CommandChip command={command.command} tone='quiet' />
														</div>
													))}
												</div>
											</InstallRow>
										)}
									</Fragment>
								);
							})}
							<InstallRow
								title={t(messages.installNpm)}
								detail={t(messages.installNpmNote)}
								group='npm'
							>
								<CommandChip command={NPM_INSTALL_COMMAND} tone='quiet' />
							</InstallRow>
							<InstallRow
								title={t(messages.installStart)}
								detail={t(START_NOTE[offered])}
								group='all'
							>
								<CommandChip command='orchbee' tone='quiet' />
							</InstallRow>
							{platforms.map(({ platform }) =>
								platform.install.note === undefined ? null : (
									<p className={styles['note']} data-install-note={platform.id} key={platform.id}>
										{t(shellMessages[platform.install.note], richText)}
									</p>
								),
							)}
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
