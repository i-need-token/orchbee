import {
	useOrchbeeIntl,
	websiteHomeMessages as messages,
	websiteShellMessages as shellMessages,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import { pagePath } from '../../../../pages';
import {
	offeredPlatforms,
	type OfferedPlatforms,
	REPOSITORY_URL,
	type Release,
} from '../../../../release';
import { GetOrchbee } from '../../../get-orchbee/GetOrchbee';
import { LinkButton } from '../../../link-button/LinkButton';
import { SessionScene } from '../../../session-scene/SessionScene';
import { HeroStage } from './components/hero-stage/HeroStage';
import styles from './Hero.module.css';

// The line of facts, by the platforms besides Windows the release offers.
const FACTS = {
	windows: messages.facts,
	macos: messages.factsMacos,
	linux: messages.factsLinux,
	all: messages.factsAll,
} as const satisfies Record<OfferedPlatforms, unknown>;

// The top of the home page: the headline, the concise lead, the Download button that opens the
// download dialog (before a release, the source build), the line of facts under it, and the drawn
// Session stage. The facts name macOS and Linux only while the release offers them.
export function Hero({
	locale,
	release,
}: {
	locale: OrchbeeLocale;
	release: Release;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<div className={styles['column']}>
			<SessionScene>
				<div className={styles['head']}>
					<div>
						<p className={styles['beta']}>{t(messages.beta)}</p>
						<h1 className={styles['title']}>{t(messages.title)}</h1>
					</div>
					<div>
						<p className={styles['lead']}>{t(messages.lead)}</p>
						{release.published ? (
							<div className={styles['actions']} data-page-download=''>
								<LinkButton
									href={`${pagePath('home', locale)}#install`}
									icon='download'
									downloadTrigger
								>
									{t(shellMessages.navDownload)}
								</LinkButton>
							</div>
						) : (
							<GetOrchbee locale={locale} release={release} offer='both' />
						)}
						<p className={styles['facts']}>
							{t(FACTS[offeredPlatforms(release)], {
								repo: (chunks: readonly ReactNode[]) => (
									<a
										className={styles['repo']}
										href={REPOSITORY_URL}
										target='_blank'
										rel='noreferrer'
									>
										{chunks}
									</a>
								),
							})}
						</p>
					</div>
				</div>
				<HeroStage />
			</SessionScene>
		</div>
	);
}
