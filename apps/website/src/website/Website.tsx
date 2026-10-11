import { useOrchbeeIntl, websiteMessages as messages, type OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement } from 'react';
import { Showcase } from './components/showcase/Showcase';
import styles from './Website.module.css';

const REPOSITORY_URL = 'https://github.com/i-need-token/orchbee';

export function Website({ locale }: { locale: OrchbeeLocale }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const home = locale === 'zh-CN' ? '/zh-CN/' : '/';
	const alternate = locale === 'zh-CN' ? '/' : '/zh-CN/';
	const features = [
		{ number: '01', title: messages.featureOneTitle, body: messages.featureOneBody },
		{ number: '02', title: messages.featureTwoTitle, body: messages.featureTwoBody },
		{ number: '03', title: messages.featureThreeTitle, body: messages.featureThreeBody },
	];
	const questions = [
		{ question: messages.faqOneQ, answer: messages.faqOneA },
		{ question: messages.faqTwoQ, answer: messages.faqTwoA },
		{ question: messages.faqThreeQ, answer: messages.faqThreeA },
		{ question: messages.faqFourQ, answer: messages.faqFourA },
	];
	return (
		<div className={styles['page']}>
			<a className={styles['skip']} href='#main'>
				{t(messages.skip)}
			</a>
			<header className={styles['header']}>
				<div className={styles['headerInner']}>
					<a className={styles['brand']} href={home} aria-label='Orchbee'>
						<img src='/icon.svg' width='30' height='30' alt='' />
						Orchbee
					</a>
					<nav className={styles['navigation']} aria-label={t(messages.navigation)}>
						<a href='#features'>{t(messages.features)}</a>
						<a href='#workflow'>{t(messages.workflowNav)}</a>
						<a href='#faq'>{t(messages.faq)}</a>
					</nav>
					<div className={styles['preferences']}>
						<a
							href={alternate}
							hrefLang={locale === 'zh-CN' ? 'en-US' : 'zh-CN'}
							lang={locale === 'zh-CN' ? 'en-US' : 'zh-CN'}
							data-language-link=''
						>
							{t(messages.language)}
						</a>
						<select
							className={styles['appearance']}
							aria-label={t(messages.appearance)}
							data-appearance=''
							defaultValue='system'
							hidden
						>
							<option value='system'>{t(messages.system)}</option>
							<option value='light'>{t(messages.light)}</option>
							<option value='dark'>{t(messages.dark)}</option>
						</select>
						<a className={styles['headerCta']} href={REPOSITORY_URL}>
							GitHub <span aria-hidden='true'>↗︎</span>
						</a>
					</div>
					<details className={styles['mobileMenu']} data-mobile-menu=''>
						<summary>{t(messages.menu)}</summary>
						<nav aria-label={t(messages.navigation)}>
							<a href='#features'>{t(messages.features)}</a>
							<a href='#workflow'>{t(messages.workflowNav)}</a>
							<a href='#faq'>{t(messages.faq)}</a>
							<a href={REPOSITORY_URL}>{t(messages.source)}</a>
						</nav>
					</details>
				</div>
			</header>
			<main id='main'>
				<section className={styles['hero']}>
					<div className={styles['heroCopy']}>
						<a className={styles['announcement']} href='#get-started'>
							<span className={styles['statusDot']} aria-hidden='true' />
							{t(messages.early)} <span aria-hidden='true'>↗︎</span>
						</a>
						<h1>
							{t(messages.heroTitle)}
							<br />
							<span>{t(messages.heroAccent)}</span>
						</h1>
						<p className={styles['heroDescription']}>{t(messages.heroDescription)}</p>
						<div className={styles['heroActions']}>
							<a className={styles['primary']} href={REPOSITORY_URL}>
								{t(messages.explore)} <span aria-hidden='true'>↗︎</span>
							</a>
							<a className={styles['secondary']} href='#showcase'>
								{t(messages.see)} <span aria-hidden='true'>↓</span>
							</a>
						</div>
					</div>
					<div className={styles['heroVisual']} aria-hidden='true'>
						<div className={styles['orbit']} />
						<img
							src='/agent-sculpture.png'
							width='1254'
							height='1254'
							alt=''
							fetchPriority='high'
						/>
						<span className={styles['visualIndex']}>ORCHBEE / 001</span>
					</div>
				</section>
				<div className={styles['content']}>
					<section className={styles['agents']} aria-label={t(messages.agentsLabel)}>
						<p>{t(messages.agentsLabel)}</p>
						<ul>
							<li>Claude Code</li>
							<li>Codex</li>
							<li>OpenCode</li>
							<li>Gemini CLI</li>
						</ul>
						<span>{t(messages.agentsNote)}</span>
					</section>
					<section className={styles['features']} id='features'>
						<div className={styles['sectionHeading']}>
							<div>
								<p className={styles['eyebrow']}>{t(messages.featuresKicker)}</p>
								<h2>{t(messages.featuresTitle)}</h2>
							</div>
							<p>{t(messages.featuresBody)}</p>
						</div>
						<Showcase />
						<div className={styles['featureGrid']}>
							{features.map((feature) => (
								<article key={feature.number}>
									<span className={styles['featureNumber']}>{feature.number}</span>
									<h3>{t(feature.title)}</h3>
									<p>{t(feature.body)}</p>
								</article>
							))}
						</div>
					</section>
					<section className={styles['workflow']} id='workflow'>
						<div className={styles['workflowCopy']}>
							<p className={styles['eyebrow']}>{t(messages.workflowKicker)}</p>
							<h2>{t(messages.workflowTitle)}</h2>
							<p>{t(messages.workflowBody)}</p>
							<a className={styles['textLink']} href={REPOSITORY_URL}>
								{t(messages.workflowLink)} <span aria-hidden='true'>↗︎</span>
							</a>
						</div>
						<div className={styles['commandScene']}>
							<div className={styles['commandCard']} data-theme='dark'>
								<div className={styles['commandBar']}>
									<span>orchbee / cli</span>
									<span>{t(messages.workflowExample)}</span>
								</div>
								<div className={styles['commands']}>
									<p># {t(messages.workflowCommandComment)}</p>
									<code>
										<span>$</span> orchbee agent read reviewer
									</code>
									<div className={styles['commandOutput']}>
										<span>reviewer</span>
										<p>{t(messages.workflowOutput)}</p>
									</div>
									<p># {t(messages.workflowCommandCommentTwo)}</p>
									<code>
										<span>$</span> orchbee agent prompt reviewer
										<br />
										&nbsp; &quot;{t(messages.workflowPrompt)}&quot;
									</code>
								</div>
								<div className={styles['commandFoot']}>
									<span className={styles['statusDot']} />
									{t(messages.workflowFoot)}
								</div>
							</div>
						</div>
					</section>
					<section className={styles['privacy']} id='principles'>
						<span className={styles['privacyMark']} aria-hidden='true'>
							/
						</span>
						<h3>{t(messages.privacyTitle)}</h3>
						<p>{t(messages.privacyBody)}</p>
					</section>
					<section className={styles['faq']} id='faq'>
						<div>
							<p className={styles['eyebrow']}>{t(messages.faq)}</p>
							<h2>{t(messages.faqTitle)}</h2>
						</div>
						<div className={styles['questions']}>
							{questions.map((item) => (
								<details key={item.question.id}>
									<summary>
										{t(item.question)}
										<span aria-hidden='true'>+</span>
									</summary>
									<p>{t(item.answer)}</p>
								</details>
							))}
						</div>
					</section>
					<section className={styles['closing']} id='get-started'>
						<div>
							<p className={styles['eyebrow']}>{t(messages.early)}</p>
							<h2>{t(messages.closingTitle)}</h2>
							<p>{t(messages.closingBody)}</p>
						</div>
						<a className={styles['primary']} href={REPOSITORY_URL}>
							{t(messages.source)} <span aria-hidden='true'>↗︎</span>
						</a>
					</section>
				</div>
			</main>
			<footer className={styles['footer']}>
				<div>
					<a className={styles['brand']} href={home}>
						<img src='/icon.svg' width='30' height='30' alt='' />
						Orchbee
					</a>
					<p>{t(messages.footerTagline)}</p>
				</div>
				<div className={styles['footerLinks']}>
					<a href='#features'>{t(messages.features)}</a>
					<a href={REPOSITORY_URL}>GitHub ↗︎</a>
					<a href='#principles'>{t(messages.principles)}</a>
				</div>
				<p>© Orchbee</p>
			</footer>
		</div>
	);
}
