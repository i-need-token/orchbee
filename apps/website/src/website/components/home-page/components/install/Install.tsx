import {
	useOrchbeeIntl,
	websiteHomeMessages as messages,
	websiteShellMessages as shellMessages,
} from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import {
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
	children,
}: {
	title: ReactNode;
	detail?: ReactNode;
	children: ReactNode;
}): ReactElement {
	return (
		<div className={styles['row']}>
			<div>
				<b className={styles['title']}>{title}</b>
				{detail === undefined ? null : <small className={styles['detail']}>{detail}</small>}
			</div>
			{children}
		</div>
	);
}

// How to install Orchbee. Once a release is published: the Windows installer, the npm package and
// how to start, with the release named; before that, the source build.
export function Install({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['section']} id='install'>
			<div className={styles['install']} data-reveal=''>
				<div>
					<SectionHead
						layout='stack'
						kicker={t(messages.installKicker)}
						title={t(messages.installTitle)}
						lead={t(messages.installLead)}
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
								title={t(messages.installApp)}
								detail={t(shellMessages.installerNote, richText)}
							>
								<LinkButton href={release.installerUrl} icon='download'>
									{t(shellMessages.downloadForWindows)}
								</LinkButton>
							</InstallRow>
							<InstallRow title={t(messages.installNpm)} detail={t(messages.installNpmNote)}>
								<CommandChip command={NPM_INSTALL_COMMAND} tone='quiet' />
							</InstallRow>
							<InstallRow title={t(messages.installStart)} detail={t(messages.installStartNote)}>
								<CommandChip command='orchbee' tone='quiet' />
							</InstallRow>
							<p className={styles['note']}>{t(messages.installNote)}</p>
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
