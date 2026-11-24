import { Button } from '@orchbee/ui/components';
import { useId, useSyncExternalStore, type KeyboardEvent, type ReactNode } from 'react';

import type { WorkbenchOtherMachines } from '../../types/workbench-other-machines';
import styles from './WorkbenchMachineCard.module.css';

/**
 * Stands over the terminals while a chosen machine is not connected, as the TUI's card does: its
 * state's title and lines, the state's action and Not now (Esc). It holds the pointer and takes the
 * keyboard when it shows, so nothing reaches the terminals beneath it.
 */
export function WorkbenchMachineCard({
	otherMachines,
}: {
	readonly otherMachines: WorkbenchOtherMachines;
}): ReactNode {
	const titleId = useId();
	const read = () => otherMachines.getSnapshot().card;
	const card = useSyncExternalStore(otherMachines.subscribe, read, read);
	if (card === null) return null;
	const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
		if (event.key !== 'Escape') return;
		event.preventDefault();
		event.stopPropagation();
		otherMachines.dismiss();
	};
	return (
		<div
			aria-labelledby={titleId}
			className={styles['root']}
			data-slot='workbench-machine-card'
			onKeyDown={handleKeyDown}
			role='alertdialog'
		>
			<div className={styles['card']}>
				<p className={styles['title']} id={titleId}>
					{card.title}
				</p>
				{card.lines.map((line) => (
					<p className={styles['line']} key={line}>
						{line}
					</p>
				))}
				<div className={styles['buttons']}>
					{card.action === null ? null : (
						<Button
							autoFocus
							data-slot='workbench-machine-card-action'
							onClick={() => otherMachines.act()}
							size='sm'
						>
							{card.action}
						</Button>
					)}
					<Button
						autoFocus={card.action === null}
						data-slot='workbench-machine-card-not-now'
						onClick={() => otherMachines.dismiss()}
						size='sm'
						variant='outline'
					>
						{card.notNow}
					</Button>
				</div>
			</div>
		</div>
	);
}
