import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { AccentText } from '../accent-text/AccentText';
import { CommandBlock } from '../command-block/CommandBlock';
import { CopyButton } from '../copy-button/CopyButton';
import { DownloadIcon } from '../download-icon/DownloadIcon';
import { GithubIcon } from '../github-icon/GithubIcon';
import { Install } from '../install/Install';
import { LinkButton } from '../link-button/LinkButton';
import { ProductPicture } from '../product-picture/ProductPicture';
import { SectionHeading } from '../section-heading/SectionHeading';
import { NPM_INSTALL_COMMAND, REPOSITORY_URL, type Release } from '../../release';
import styles from './Home.module.css';

// The commands of the README's agent-to-agent example, verbatim.
const AGENT_COMMANDS = [
	'orchbee agent start codex --split right',
	'orchbee agent list',
	'orchbee agent prompt @panel:0x36 "Review the current changes"',
	'orchbee agent wait @panel:0x36 --until idle --timeout 120000',
	'orchbee agent read @panel:0x36 --source recent --lines 40',
] as const;

// Display names of agent CLIs that Orchbee recognizes, as its sidebar shows them.
const AGENT_KINDS = [
	'Claude Code',
	'Codex',
	'Gemini CLI',
	'OpenCode',
	'Pi',
	'GitHub Copilot CLI',
	'Qwen Code',
	'Kimi',
	'Amp',
	'Grok',
	'Droid',
	'Cline',
] as const;

export function Home({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<>
			<section className={styles['hero']} aria-labelledby='hero-title'>
				<div className={styles['wrap']}>
					<p className={styles['status']}>
						<span className={styles['statusDot']} aria-hidden='true' />
						{release.published
							? t(messages.statusReleased, { version: release.version })
							: t(messages.statusUnreleased)}
					</p>
					<h1 className={styles['heroTitle']} id='hero-title'>
						<span className={styles['line']}>{t(messages.heroTitleLead)}</span>
						<AccentText>{t(messages.heroTitleAccent)}</AccentText>
					</h1>
					<p className={styles['heroLead']}>{t(messages.heroLead)}</p>
					<div
						className={styles['actions']}
						data-release={release.published ? 'published' : 'unpublished'}
					>
						{release.published ? (
							<>
								<LinkButton href={release.installerUrl}>
									<DownloadIcon />
									{t(messages.downloadForWindows)}
								</LinkButton>
								<p className={styles['commandPill']}>
									<span className={styles['prompt']} aria-hidden='true'>
										$
									</span>
									<code>{NPM_INSTALL_COMMAND}</code>
									<CopyButton text={NPM_INSTALL_COMMAND} />
								</p>
							</>
						) : (
							<>
								<LinkButton href={REPOSITORY_URL}>
									<GithubIcon />
									{t(messages.viewOnGitHub)}
								</LinkButton>
								<LinkButton href='#install' variant='secondary'>
									{t(messages.buildFromSource)}
								</LinkButton>
							</>
						)}
					</div>
					<p className={styles['platforms']}>
						{release.published ? t(messages.platformsReleased) : t(messages.platformsUnreleased)}
					</p>
					<div className={styles['stage']}>
						<ProductPicture picture='hero' alt={t(messages.heroPictureAlt)} priority />
					</div>
				</div>
			</section>
			<section className={styles['section']} id='terminal'>
				<div className={styles['wrap']}>
					<SectionHeading
						number='01'
						label={t(messages.terminalLabel)}
						lead={t(messages.terminalTitleLead)}
						accent={t(messages.terminalTitleAccent)}
						body={t(messages.terminalBody)}
					/>
					<div className={styles['points']}>
						<div>
							<h3>{t(messages.terminalCoreTitle)}</h3>
							<p>{t(messages.terminalCoreBody)}</p>
						</div>
						<div>
							<h3>{t(messages.terminalScreenTitle)}</h3>
							<p>{t(messages.terminalScreenBody)}</p>
						</div>
						<div>
							<h3>{t(messages.terminalGpuTitle)}</h3>
							<p>{t(messages.terminalGpuBody)}</p>
						</div>
					</div>
				</div>
			</section>
			<section className={styles['section']} id='sessions'>
				<div className={styles['wrap']}>
					<SectionHeading
						number='02'
						label={t(messages.multiplexLabel)}
						lead={t(messages.multiplexTitleLead)}
						accent={t(messages.multiplexTitleAccent)}
						body={t(messages.multiplexBody)}
					/>
					<div className={styles['visual']}>
						<ProductPicture
							picture='multiplex'
							alt={t(messages.multiplexPictureAlt)}
							caption={t(messages.multiplexCaption)}
						/>
					</div>
				</div>
			</section>
			<section className={styles['section']} id='orchestrate'>
				<div className={styles['wrap']}>
					<SectionHeading
						number='03'
						label={t(messages.orchestrateLabel)}
						lead={t(messages.orchestrateTitleLead)}
						accent={t(messages.orchestrateTitleAccent)}
						body={t(messages.orchestrateBody)}
					/>
					<div className={styles['visual']}>
						<ProductPicture
							picture='agents'
							alt={t(messages.orchestratePictureAlt)}
							caption={t(messages.orchestrateCaption)}
						/>
					</div>
					<div className={styles['commands']}>
						<CommandBlock
							title={t(messages.orchestrateCommandsTitle)}
							lines={AGENT_COMMANDS.map((text) => ({ kind: 'command', text }) as const)}
						/>
						<div className={styles['commandNotes']}>
							<p>{t(messages.orchestrateHandleNote)}</p>
							<p className={styles['planned']}>
								<span className={styles['tag']}>{t(messages.remoteTag)}</span>
								{t(messages.remoteBody)}
							</p>
						</div>
					</div>
				</div>
			</section>
			<section className={styles['section']} id='agents'>
				<div className={styles['wrap']}>
					<SectionHeading
						number='04'
						label={t(messages.agentsLabel)}
						lead={t(messages.agentsTitleLead)}
						accent={t(messages.agentsTitleAccent)}
						body={t(messages.agentsBody)}
					/>
					<div className={styles['visual']}>
						<ProductPicture
							picture='desktop'
							alt={t(messages.agentsPictureAlt)}
							caption={t(messages.agentsCaption)}
						/>
					</div>
					<ul className={styles['legend']} aria-label={t(messages.agentStates)}>
						<li>
							<span className={styles['working']} aria-hidden='true' />
							{t(messages.stateWorking)}
						</li>
						<li>
							<span className={styles['attention']} aria-hidden='true' />
							{t(messages.stateAttention)}
						</li>
						<li>
							<span className={styles['done']} aria-hidden='true' />
							{t(messages.stateDone)}
						</li>
					</ul>
					<div className={styles['kinds']}>
						<h3>{t(messages.agentKinds)}</h3>
						<ul>
							{AGENT_KINDS.map((kind) => (
								<li key={kind}>{kind}</li>
							))}
						</ul>
					</div>
				</div>
			</section>
			<section className={styles['section']} id='install'>
				<div className={styles['wrap']}>
					<Install number='05' release={release} />
				</div>
			</section>
			<section className={styles['outro']} aria-labelledby='outro-title'>
				<div className={styles['wrap']}>
					<h2 className={styles['outroTitle']} id='outro-title'>
						<span className={styles['line']}>{t(messages.outroTitleLead)}</span>
						<AccentText>{t(messages.outroTitleAccent)}</AccentText>
					</h2>
					<div className={styles['mark']} aria-hidden='true'>
						<img src='/icon.svg' width='84' height='84' alt='' />
					</div>
					<div className={styles['outroActions']}>
						{release.published ? (
							<LinkButton href={release.installerUrl}>
								<DownloadIcon />
								{t(messages.downloadForWindows)}
							</LinkButton>
						) : (
							<LinkButton href='#install'>{t(messages.buildFromSource)}</LinkButton>
						)}
						<LinkButton href={REPOSITORY_URL} variant='secondary'>
							<GithubIcon />
							GitHub
						</LinkButton>
					</div>
				</div>
			</section>
		</>
	);
}
