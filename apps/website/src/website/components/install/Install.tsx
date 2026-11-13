import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import { Fragment, type ReactElement, type ReactNode } from 'react';

import {
	BUILD_GUIDE_URL,
	NPM_INSTALL_COMMAND,
	REPOSITORY_URL,
	SOURCE_COMMANDS,
	releaseNotesUrl,
	type Release,
} from '../../release';
import { CopyButton } from '../copy-button/CopyButton';
import { Icon } from '../icon/Icon';
import { PillButton } from '../pill-button/PillButton';
import { SectionHeading } from '../section-heading/SectionHeading';
import { WindowFrame } from '../window-frame/WindowFrame';
import styles from './Install.module.css';

type Os = 'mac' | 'win' | 'linux';

const OS_TABS: readonly (readonly [Os, string, string])[] = [
	['mac', 'macOS', 'zsh'],
	['win', 'Windows', 'PowerShell'],
	['linux', 'Linux', 'bash'],
];

// How to get Orchbee, from the release switch: the source build while nothing is released, the
// Windows installer and npm once a release exists. Other systems say they are in progress.
export function Install({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const comment = (text: string): ReactElement => <div className={styles['comment']}># {text}</div>;
	const command = (text: string, copy?: string): ReactElement => (
		<div className={styles['command']}>
			<span className={styles['prompt']}>PS&gt;</span>
			<code>{text}</code>
			{copy === undefined ? null : <CopyButton text={copy} showLabel tone='terminal' />}
		</div>
	);
	const windows: ReactNode = release.published ? (
		<>
			{comment(t(messages.commentInstall))}
			{command(NPM_INSTALL_COMMAND, NPM_INSTALL_COMMAND)}
			{comment(t(messages.commentOpen))}
			{command('orchbee')}
			{comment(t(messages.commentStartAgent))}
			{command('orchbee agent start claude-code')}
		</>
	) : (
		<>
			{comment(t(messages.commentSource))}
			{SOURCE_COMMANDS.map((line, index) => (
				<Fragment key={line}>
					{command(line, index === 0 ? SOURCE_COMMANDS.join('\n') : undefined)}
				</Fragment>
			))}
			{comment(t(messages.commentDesktop))}
			{command('pnpm desktop')}
		</>
	);
	return (
		<section className={styles['scene']} id='install' data-scene='6'>
			<div className={styles['inner']}>
				<div>
					<SectionHeading
						number='06'
						label={t(messages.sceneInstall)}
						lead={t(
							release.published
								? messages.installTitleLeadReleased
								: messages.installTitleLeadSource,
						)}
						accent={t(
							release.published
								? messages.installTitleAccentReleased
								: messages.installTitleAccentSource,
						)}
					>
						{t(release.published ? messages.installBodyReleased : messages.installBodySource)}
					</SectionHeading>
					<div className={styles['card']}>
						<div className={styles['cardHead']}>
							<span>{t(messages.desktopApp)}</span>
							{release.published ? (
								<a className={styles['notes']} href={releaseNotesUrl(release.version)}>
									{release.version} · {t(messages.releaseNotes)}
								</a>
							) : null}
						</div>
						<div className={styles['files']}>
							{release.published ? (
								<PillButton variant='glass' size='small' icon='dl' href={release.installerUrl}>
									x64<small>.exe</small>
								</PillButton>
							) : (
								<PillButton variant='glass' size='small' icon='gh' href={BUILD_GUIDE_URL}>
									{t(messages.buildGuide)}
								</PillButton>
							)}
						</div>
						<p className={styles['requirements']}>
							{t(release.published ? messages.requirementsReleased : messages.requirementsSource)}
						</p>
					</div>
					<a className={styles['github']} href={REPOSITORY_URL}>
						<Icon name='gh' size={17} />
						<span>{t(messages.githubLink)}</span>
						<span aria-hidden='true'>→</span>
					</a>
				</div>
				<div data-install=''>
					<WindowFrame
						title={
							<>
								{OS_TABS.map(([os, , shell]) => (
									<span key={os} data-os-title={os} hidden={os !== 'win'}>
										{shell}
									</span>
								))}
							</>
						}
						barEnd={
							<div className={styles['tabs']} role='group' aria-label={t(messages.operatingSystem)}>
								{OS_TABS.map(([os, name]) => (
									<button key={os} type='button' data-os-tab={os} aria-pressed={os === 'win'}>
										{name}
									</button>
								))}
							</div>
						}
					>
						{OS_TABS.map(([os, name]) => (
							<div className={styles['body']} key={os} data-os-panel={os} hidden={os !== 'win'}>
								{os === 'win' ? windows : comment(t(messages.commentInProgress, { os: name }))}
							</div>
						))}
					</WindowFrame>
				</div>
			</div>
		</section>
	);
}
