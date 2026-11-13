import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import {
	NPM_INSTALL_COMMAND,
	REPOSITORY_URL,
	SOURCE_COMMANDS,
	releaseNotesUrl,
	type Release,
} from '../../release';
import { CommandBlock } from '../command-block/CommandBlock';
import { DownloadIcon } from '../download-icon/DownloadIcon';
import { LinkButton } from '../link-button/LinkButton';
import { SectionHeading } from '../section-heading/SectionHeading';
import styles from './Install.module.css';

export function Install({ number, release }: { number: string; release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	if (!release.published) {
		return (
			<div className={styles['layout']} data-release='unpublished'>
				<div>
					<SectionHeading
						number={number}
						label={t(messages.installLabel)}
						lead={t(messages.installSourceTitleLead)}
						accent={t(messages.installSourceTitleAccent)}
						body={t(messages.installSourceBody)}
					/>
					<p className={styles['requirements']}>{t(messages.installPrerequisites)}</p>
					<a className={styles['link']} href={`${REPOSITORY_URL}#install`}>
						{t(messages.installReadme)} <span aria-hidden='true'>→</span>
					</a>
				</div>
				<CommandBlock
					title='PowerShell'
					lines={[
						{ kind: 'comment', text: t(messages.installCommentSource) },
						...SOURCE_COMMANDS.map((text) => ({ kind: 'command', text }) as const),
						{ kind: 'comment', text: t(messages.installCommentDesktop) },
						{ kind: 'command', text: 'pnpm desktop' },
					]}
				/>
			</div>
		);
	}
	return (
		<div className={styles['layout']} data-release='published'>
			<div>
				<SectionHeading
					number={number}
					label={t(messages.installLabel)}
					lead={t(messages.installReleaseTitleLead)}
					accent={t(messages.installReleaseTitleAccent)}
					body={t(messages.installReleaseBody, { version: release.version })}
				/>
				<div className={styles['download']}>
					<p className={styles['downloadTitle']}>{t(messages.installDesktopTitle)}</p>
					<div className={styles['downloadAction']}>
						<LinkButton href={release.installerUrl}>
							<DownloadIcon />
							{t(messages.installDownload)}
						</LinkButton>
					</div>
					<p className={styles['note']}>{t(messages.installInstallerNote)}</p>
				</div>
				<a className={styles['link']} href={releaseNotesUrl(release.version)}>
					{t(messages.installReleaseNotes)} <span aria-hidden='true'>→</span>
				</a>
			</div>
			<CommandBlock
				title='PowerShell'
				lines={[
					{ kind: 'comment', text: t(messages.installCommentNpm) },
					{ kind: 'command', text: NPM_INSTALL_COMMAND, copyable: true },
					{ kind: 'comment', text: t(messages.installCommentRun) },
					{ kind: 'command', text: 'orchbee' },
				]}
			/>
		</div>
	);
}
