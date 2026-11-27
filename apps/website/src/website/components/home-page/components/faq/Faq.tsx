import { useOrchbeeIntl, websiteHomeMessages as messages } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import {
	offeredPlatforms,
	type OfferedPlatforms,
	REPOSITORY_URL,
	type Release,
} from '../../../../release';
import { Kicker } from '../../../kicker/Kicker';
import styles from './Faq.module.css';

// The platforms answer, by the platforms besides Windows the release offers.
const PLATFORMS_ANSWER = {
	windows: messages.faqPlatformsAnswer,
	macos: messages.faqPlatformsAnswerMacos,
	linux: messages.faqPlatformsAnswerLinux,
	all: messages.faqPlatformsAnswerAll,
} as const satisfies Record<OfferedPlatforms, unknown>;

// The questions a new visitor asks before installing, each with its answer in view. The platforms
// answer names macOS and Linux only while the release offers them.
export function Faq({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const repo = (chunks: readonly ReactNode[]): ReactElement => (
		<a className={styles['link']} href={REPOSITORY_URL} target='_blank' rel='noreferrer'>
			{chunks}
		</a>
	);
	const entries = [
		{ id: 'free', question: messages.faqFreeQuestion, answer: t(messages.faqFreeAnswer, { repo }) },
		{ id: 'replace', question: messages.faqReplaceQuestion, answer: t(messages.faqReplaceAnswer) },
		{ id: 'tmux', question: messages.faqTmuxQuestion, answer: t(messages.faqTmuxAnswer) },
		{
			id: 'platforms',
			question: messages.faqPlatformsQuestion,
			answer: t(PLATFORMS_ANSWER[offeredPlatforms(release)]),
		},
		{ id: 'data', question: messages.faqDataQuestion, answer: t(messages.faqDataAnswer) },
		{ id: 'next', question: messages.faqNextQuestion, answer: t(messages.faqNextAnswer) },
	] as const;
	return (
		<section className={styles['section']} id='faq'>
			<div className={styles['layout']} data-reveal=''>
				<div>
					<Kicker>{t(messages.faqKicker)}</Kicker>
					<h2 className={styles['title']}>{t(messages.faqTitle)}</h2>
				</div>
				<dl className={styles['list']}>
					{entries.map(({ id, question, answer }) => (
						<div key={id} className={styles['entry']}>
							<dt className={styles['question']}>{t(question)}</dt>
							<dd className={styles['answer']}>{answer}</dd>
						</div>
					))}
				</dl>
			</div>
		</section>
	);
}
