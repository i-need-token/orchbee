import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import { useId, useState, type ReactNode, type RefObject } from 'react';

import type { ProjectedTerminalLaunchFailureView } from '../../types/projected-terminal-surface-view';
import styles from './ProjectedTerminalLaunchFailure.module.css';

interface ProjectedTerminalLaunchFailureProps {
	readonly failure: ProjectedTerminalLaunchFailureView;
	readonly onClose: () => void;
	readonly onCreateTab: () => void;
	/** Starts a held restore's terminal; null where this surface cannot. */
	readonly onRestore: ((directory: 'home' | 'saved') => Promise<void>) | null;
	readonly rootRef: RefObject<HTMLElement | null>;
}

/**
 * Takes keyboard focus itself rather than a button, so a key typed as it appears runs nothing.
 * Nothing here retries by itself: a person opens another Tab or closes this terminal, and a held
 * restore starts only when a person asks for it again or in the home folder.
 */
export function ProjectedTerminalLaunchFailure({
	failure,
	onClose,
	onCreateTab,
	onRestore,
	rootRef,
}: ProjectedTerminalLaunchFailureProps): ReactNode {
	const intl = useOrchbeeIntl();
	const titleId = useId();
	const [restoring, setRestoring] = useState(false);
	if (failure.kind === 'directory-unavailable') {
		const restore = (directory: 'home' | 'saved'): void => {
			if (onRestore === null || restoring) return;
			setRestoring(true);
			void onRestore(directory).finally(() => setRestoring(false));
		};
		return (
			<section
				aria-busy={restoring}
				aria-labelledby={titleId}
				className={styles['root']}
				data-slot='projected-terminal-held-restore'
				ref={rootRef}
				tabIndex={-1}
			>
				<div className={styles['content']}>
					<h2 className={styles['title']} id={titleId}>
						{intl.formatMessage(projectedTerminalMessages.heldRestoreTitle)}
					</h2>
					<code className={styles['directory']}>{failure.directory}</code>
					<div className={styles['actions']}>
						{onRestore === null ? null : (
							<>
								<Button
									data-slot='projected-terminal-held-restore-retry'
									disabled={restoring}
									onClick={() => restore('saved')}
								>
									{intl.formatMessage(projectedTerminalMessages.heldRestoreRetry)}
								</Button>
								<Button
									data-slot='projected-terminal-held-restore-home'
									disabled={restoring}
									onClick={() => restore('home')}
									variant='outline'
								>
									{intl.formatMessage(projectedTerminalMessages.heldRestoreStartInHome)}
								</Button>
							</>
						)}
						<Button
							data-slot='projected-terminal-held-restore-close'
							onClick={onClose}
							variant='outline'
						>
							{intl.formatMessage(projectedTerminalMessages.launchClose)}
						</Button>
					</div>
				</div>
			</section>
		);
	}
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
						{intl.formatMessage(projectedTerminalMessages.newTab)}
					</Button>
					<Button data-slot='projected-terminal-launch-close' onClick={onClose} variant='outline'>
						{intl.formatMessage(projectedTerminalMessages.launchClose)}
					</Button>
				</div>
			</div>
		</section>
	);
}
