import { fileOverlayMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	Button,
	Kbd,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import { useRef, type ReactNode } from 'react';

import {
	useWorkbenchKeyboardScope,
	type WorkbenchKeyboardScope,
} from '../../../../../../workbench-keyboard-scopes';
import {
	useFileOverlayHostCommands,
	useFileOverlayHostShortcutLabels,
	useFileOverlayHostStore,
} from '../../contexts/file-overlay-host-store';
import styles from './FileOverlayFrame.module.css';

/** Focus anywhere in the frame reaches the overlay's keys, then the File Bar's. */
const FILE_OVERLAY_KEYBOARD_SCOPE: WorkbenchKeyboardScope = { kind: 'file-overlay' };

/**
 * The overlay's frame: a region landmark, never a dialog, since the rest of the window stays
 * operable. Its row ends with Expand, or Restore while the tier is full, above the content.
 */
export function FileOverlayFrame(): ReactNode {
	const intl = useOrchbeeIntl();
	const rootReference = useRef<HTMLElement>(null);
	useWorkbenchKeyboardScope(rootReference, FILE_OVERLAY_KEYBOARD_SCOPE);
	const full = useFileOverlayHostStore((state) => state.mode.tier === 'full');
	const commands = useFileOverlayHostCommands();
	const shortcutLabels = useFileOverlayHostShortcutLabels();
	const toggleLabel = intl.formatMessage(
		full ? fileOverlayMessages.restore : fileOverlayMessages.expand,
	);
	return (
		<section
			aria-label={intl.formatMessage(fileOverlayMessages.label)}
			className={styles['root']}
			data-slot='file-overlay-frame'
			ref={rootReference}
		>
			<div className={styles['row']} data-slot='file-overlay-row'>
				<div className={styles['tabs']} />
				<div className={styles['titleActions']}>
					<Tooltip>
						<TooltipTrigger
							render={
								<Button
									aria-label={toggleLabel}
									className={styles['action']}
									data-slot='file-overlay-toggle-full'
									size='icon-sm'
									variant='ghost'
									onClick={commands.toggleFull}
								/>
							}
						>
							<ProductIcon name={full ? 'screen-normal' : 'screen-full'} />
						</TooltipTrigger>
						<TooltipContent align='end' side='bottom'>
							{toggleLabel}
							{shortcutLabels.toggleFull === undefined ? null : (
								<Kbd>{shortcutLabels.toggleFull}</Kbd>
							)}
						</TooltipContent>
					</Tooltip>
				</div>
			</div>
			<div className={styles['content']} data-slot='file-overlay-content' />
		</section>
	);
}
