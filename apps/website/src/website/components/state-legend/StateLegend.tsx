import { useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { StateGlyph, type AgentState } from '../state-glyph/StateGlyph';
import { AGENT_STATE_WORDS } from '../state-glyph/state-words';
import styles from './StateLegend.module.css';

/** The Agent state glyphs with the product's word for each, held still. */
export function StateLegend({ states }: { states: readonly AgentState[] }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<span className={styles['legend']}>
			{states.map((state) => (
				<span key={state} className={styles['entry']}>
					<StateGlyph state={state} />
					<span>{t(AGENT_STATE_WORDS[state])}</span>
				</span>
			))}
		</span>
	);
}
