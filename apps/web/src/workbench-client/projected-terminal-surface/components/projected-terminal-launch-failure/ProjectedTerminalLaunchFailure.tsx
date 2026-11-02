import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import { useId, type ReactNode, type RefObject } from 'react';

import type { ProjectedTerminalLaunchFailureView } from '../../types/projected-terminal-surface-view';
import styles from './ProjectedTerminalLaunchFailure.module.css';

interface ProjectedTerminalLaunchFailureProps {
	readonly failure: ProjectedTerminalLaunchFailureView;
	readonly onClose: () => void;
	readonly onCreateTab: () => void;
	readonly rootRef: RefObject<HTMLElement | null>;
}

/**
 * Takes keyboard focus itself rather than a button, so a key typed as it appears runs nothing.
 * Nothing here retries: a person opens another Tab or closes this terminal.
 */
export function ProjectedTerminalLaunchFailure({
	failure,
	onClose,
	onCreateTab,
	rootRef,
}: ProjectedTerminalLaunchFailureProps): ReactNode {
	const intl = useOrchbeeIntl();
	const titleId = useId();
	let reason: string | null = null;
	if (failure.kind === 'failed') {
		reason = intl.formatMessage(
			failure.executable === null
				? projectedTerminalMessages.launchShellFailed
				: projectedTerminalMessages.launchExecutableFailed,
		);
	}
	return (
		<section
			aria-labelledby={titleId}
			className={styles['root']}
			data-slot='projected-terminal-launch-failure'
			ref={rootRef}
			tabIndex={-1}
		>
			<div className={styles['content']}>
				<h2 className={styles['title']} id={titleId}>
					{intl.formatMessage(
						failure.kind === 'failed'
							? projectedTerminalMessages.launchFailed
							: projectedTerminalMessages.launchUnavailable,
					)}
				</h2>
				{reason === null ? null : <p className={styles['reason']}>{reason}</p>}
				{failure.kind === 'failed' && failure.executable !== null ? (
					<code className={styles['executable']}>{failure.executable}</code>
				) : null}
				<div className={styles['actions']}>
					<Button data-slot='projected-terminal-launch-new-tab' onClick={onCreateTab}>
						{intl.formatMessage(projectedTerminalMessages.launchNewTab)}
					</Button>
					<Button data-slot='projected-terminal-launch-close' onClick={onClose} variant='outline'>
						{intl.formatMessage(projectedTerminalMessages.launchClose)}
					</Button>
				</div>
			</div>
		</section>
	);
}
