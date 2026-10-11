import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';
import styles from './Showcase.module.css';

export function Showcase(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['showcase']} id='showcase' aria-label={t(messages.demoLabel)}>
			<figure className={styles['figure']}>
				<div className={styles['window']} data-theme='dark'>
					<div className={styles['titlebar']}>
						<span className={styles['dots']} aria-hidden='true'>
							<i />
							<i />
							<i />
						</span>
						<span>
							orchbee<span className={styles['slash']}>/</span>
							{t(messages.demoWorkspace)}
						</span>
						<span className={styles['local']}>
							<i />
							{t(messages.local)}
						</span>
					</div>
					<div className={styles['workbench']}>
						<aside className={styles['sidebar']}>
							<div className={styles['sideHeading']}>{t(messages.workspace)}</div>
							<div className={styles['sideActive']}>
								<span aria-hidden='true'>⌘</span>
								{t(messages.demoWorkspace)}
							</div>
							<div className={styles['sideItem']}>
								<span aria-hidden='true'>◇</span>
								{t(messages.demoReview)}
							</div>
							<div className={styles['sideItem']}>
								<span aria-hidden='true'>◇</span>
								{t(messages.demoDocs)}
							</div>
							<div className={styles['sideHeading']}>{t(messages.project)}</div>
							<div className={styles['sideItem']}>
								<span aria-hidden='true'>⌑</span>orchbee
							</div>
							<div className={styles['branch']}>main</div>
							<div className={styles['sidebarBottom']}>
								<span className={styles['dot']} />
								{t(messages.local)}
							</div>
						</aside>
						<div className={styles['canvas']}>
							<div className={styles['agentGrid']} id='demo-agents' data-demo-panel='agents'>
								<div className={styles['agentPane']}>
									<div className={styles['paneHeader']}>
										<span>Claude Code</span>
										<span className={styles['working']}>
											<i />
											{t(messages.running)}
										</span>
									</div>
									<div className={styles['agentBody']}>
										<div className={styles['agentIdentity']}>
											<span className={styles['agentMark']} aria-hidden='true'>
												&gt;_
											</span>
											<div>
												Claude Code<small>~/projects/orchbee</small>
											</div>
										</div>
										<p className={styles['prompt']}>
											<span aria-hidden='true'>›</span>
											{t(messages.demoTaskOne)}
										</p>
										<p>{t(messages.demoResponseOne)}</p>
										<ul className={styles['checklist']}>
											<li>{t(messages.demoCheckOne)}</li>
											<li>{t(messages.demoCheckTwo)}</li>
											<li>{t(messages.demoCheckThree)}</li>
										</ul>
										<div className={styles['editBlock']}>
											<div className={styles['editLabel']}>
												src/settings.tsx <span>+2 −1</span>
											</div>
											<pre>
												<code>
													<span className={styles['lineNumber']}> 12</span> appearance: {'{'}
													{'\n'}
													<span className={styles['removed']}>−13 theme: 'light',</span>
													{'\n'}
													<span className={styles['added']}>+13 theme: preference,</span>
													{'\n'}
													<span className={styles['added']}>+14 followSystem: true,</span>
													{'\n'}
													<span className={styles['lineNumber']}> 15</span> {'}'}
													{'\n'}
													<span className={styles['lineNumber']}> 16</span>{' '}
													savePreferences(appearance);
												</code>
											</pre>
										</div>
										<div className={styles['inputLine']}>
											<span aria-hidden='true'>›</span>
											<span>{t(messages.demoInput)}</span>
											<span className={styles['cursor']} aria-hidden='true' />
										</div>
									</div>
								</div>
								<div className={styles['agentPane']}>
									<div className={styles['paneHeader']}>
										<span>Codex</span>
										<span className={styles['ready']}>
											<i />
											{t(messages.ready)}
										</span>
									</div>
									<div className={styles['agentBody']}>
										<div className={styles['agentIdentity']}>
											<span className={styles['codexMark']} aria-hidden='true'>
												&gt;_
											</span>
											<div>
												Codex<small>~/projects/orchbee</small>
											</div>
										</div>
										<p className={styles['prompt']}>
											<span aria-hidden='true'>›</span>
											{t(messages.demoTaskTwo)}
										</p>
										<p>{t(messages.demoResponseTwo)}</p>
										<ul className={styles['checklist']}>
											<li>{t(messages.demoReviewOne)}</li>
											<li>{t(messages.demoReviewTwo)}</li>
										</ul>
										<div className={styles['reviewResult']}>
											<span aria-hidden='true'>✓</span>
											<p>{t(messages.demoReviewResult)}</p>
										</div>
										<div className={styles['shellOutput']}>
											<div>
												<span>~/projects/orchbee</span>
												<span>zsh</span>
											</div>
											<code>
												<span className={styles['shellPrompt']}>❯</span> git diff --stat{'\n'}{' '}
												src/settings.tsx | 3 ++−{'\n'} src/theme.css | 32 ++++++++++++++++++++
												{'\n\n'}
												<span className={styles['shellPrompt']}>❯</span>{' '}
												<span className={styles['cursor']} aria-hidden='true' />
											</code>
										</div>
									</div>
								</div>
							</div>
							<div
								className={styles['focusPane']}
								id='demo-terminal'
								data-demo-panel='terminal'
								hidden
							>
								<div className={styles['paneHeader']}>
									<span>{t(messages.terminal)}</span>
									<span className={styles['ready']}>
										<i />
										{t(messages.idle)}
									</span>
								</div>
								<div className={styles['focusBody']}>
									<span className={styles['focusPrompt']} aria-hidden='true'>
										&gt;_
									</span>
									<h3>{t(messages.demoPrompt)}</h3>
									<p>{t(messages.demoTerminalNote)}</p>
									<pre>
										<code>
											$ git status{'\n'}On branch main{'\n\n'}$ orchbee status
										</code>
									</pre>
								</div>
							</div>
							<div
								className={styles['focusPane']}
								id='demo-projects'
								data-demo-panel='projects'
								hidden
							>
								<div className={styles['paneHeader']}>
									<span>{t(messages.project)}</span>
									<span>orchbee</span>
								</div>
								<div className={styles['focusBody']}>
									<h3>{t(messages.demoProjectNote)}</h3>
									<div className={styles['projectRows']}>
										<div>
											<span>{t(messages.demoWorkspace)}</span>
											<code>feat/dark-mode</code>
										</div>
										<div>
											<span>{t(messages.demoReview)}</span>
											<code>review/main</code>
										</div>
										<div>
											<span>{t(messages.demoDocs)}</span>
											<code>docs/getting-started</code>
										</div>
									</div>
								</div>
							</div>
						</div>
					</div>
					<div className={styles['statusbar']}>
						<span>
							<i />
							{t(messages.local)}
						</span>
						<span>{t(messages.demoFooter)}</span>
					</div>
				</div>
				<figcaption>{t(messages.demoCaption)}</figcaption>
			</figure>
			<div className={styles['switcher']} data-showcase-controls='' hidden>
				<button
					type='button'
					aria-pressed='true'
					aria-controls='demo-agents'
					data-demo-button='agents'
				>
					<span className={styles['switchTitle']}>
						<span aria-hidden='true'>01</span>
						{t(messages.featureOneTitle)}
					</span>
					<span className={styles['switchDescription']}>{t(messages.featureOneBody)}</span>
					<span className={styles['switchCompact']}>{t(messages.parallel)}</span>
				</button>
				<button
					type='button'
					aria-pressed='false'
					aria-controls='demo-projects'
					data-demo-button='projects'
				>
					<span className={styles['switchTitle']}>
						<span aria-hidden='true'>02</span>
						{t(messages.featureTwoTitle)}
					</span>
					<span className={styles['switchDescription']}>{t(messages.featureTwoBody)}</span>
					<span className={styles['switchCompact']}>{t(messages.projects)}</span>
				</button>
				<button
					type='button'
					aria-pressed='false'
					aria-controls='demo-terminal'
					data-demo-button='terminal'
				>
					<span className={styles['switchTitle']}>
						<span aria-hidden='true'>03</span>
						{t(messages.featureThreeTitle)}
					</span>
					<span className={styles['switchDescription']}>{t(messages.featureThreeBody)}</span>
					<span className={styles['switchCompact']}>{t(messages.terminal)}</span>
				</button>
			</div>
		</section>
	);
}
