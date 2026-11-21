import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { MouseEvent, ReactNode } from 'react';

import type { WorkbenchAgentConfirmationAnswer } from '../../types/workbench-agent-confirmations';
import styles from './WorkbenchAgentConfirmationOffer.module.css';

interface WorkbenchAgentConfirmationOfferProps {
	/** The product name of the Agent the offer is about. */
	readonly agentLabel: string;
	readonly onAnswer: (answer: WorkbenchAgentConfirmationAnswer) => void;
}

/** A pointer press on the offer leaves the focus where it is: in the terminal the person types in. */
function keepFocus(event: MouseEvent<HTMLElement>): void {
	event.preventDefault();
}

/**
 * The Server's offer to answer one Agent's routine confirmations, with its three answers, where
 * Workbench notices show. It never takes focus and stays until an answer or until the Server
 * withdraws it; the TUI shows the same question on its status line and the answers in a menu.
 */
export function WorkbenchAgentConfirmationOffer({
	agentLabel,
	onAnswer,
}: WorkbenchAgentConfirmationOfferProps): ReactNode {
	const intl = useOrchbeeIntl();
	const answers = [
		['run', intl.formatMessage(workbenchMessages.agentConfirmationRun)],
		[
			'agent-type',
			intl.formatMessage(workbenchMessages.agentConfirmationAgentType, { agent: agentLabel }),
		],
		['no', intl.formatMessage(workbenchMessages.agentConfirmationNo)],
	] as const;
	return (
		<div
			className={styles['root']}
			data-slot='workbench-agent-confirmation-offer'
			onMouseDown={keepFocus}
		>
			<span className={styles['question']}>
				{intl.formatMessage(workbenchMessages.agentConfirmationOffer, { agent: agentLabel })}
			</span>
			{answers.map(([answer, label]) => (
				<Button
					key={answer}
					data-answer={answer}
					onClick={() => onAnswer(answer)}
					size='xs'
					variant='outline'
				>
					{label}
				</Button>
			))}
		</div>
	);
}
