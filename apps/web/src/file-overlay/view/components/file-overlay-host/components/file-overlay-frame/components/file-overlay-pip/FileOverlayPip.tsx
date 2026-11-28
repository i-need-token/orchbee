import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileOverlayMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	Button,
	Kbd,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import { useLayoutEffect, useRef, type ReactNode, type RefObject, type WheelEvent } from 'react';

import type {
	FileOverlayBox,
	FileOverlayPipPosition,
	FileOverlayRegions,
} from '../../../../../../../model';
import {
	useFileOverlayHostCommands,
	useFileOverlayHostDispatch,
	useFileOverlayHostFocusRequest,
	useFileOverlayHostFocusTerminal,
	useFileOverlayHostShortcutLabels,
} from '../../../../contexts/file-overlay-host-store';
import { useFileOverlayPipDrag } from './hooks/useFileOverlayPipDrag';
import { forwardFileOverlayPipWheel } from './utils/forwardFileOverlayPipWheel';
import styles from './FileOverlayPip.module.css';

/** Where the layer places picture-in-picture, and where a drag may take it. */
export interface FileOverlayPipPlacement {
	/** Where picture-in-picture shows. */
	readonly box: FileOverlayBox;
	/** Keeps where a drag left picture-in-picture. */
	readonly move: (position: FileOverlayPipPosition) => void;
	/** What the overlay lies over, which bounds a drag. */
	readonly regions: FileOverlayRegions;
}

interface FileOverlayPipProps extends FileOverlayPipPlacement {
	/** The frame's content beneath the shield, which hears the wheel turned over it. */
	readonly contentReference: RefObject<HTMLElement | null>;
	/** The frame, which a drag moves. */
	readonly frameReference: RefObject<HTMLElement | null>;
}

/**
 * Picture-in-picture's click shield over the frame's content, which shows through it. The whole of
 * picture-in-picture is one button: a click, Enter or Space shows the overlay again at its tier with
 * the keys in the active tab, a drag past the Tab Strip's threshold moves it, and the wheel scrolls
 * the content beneath. Its close control is a separate button shown on hover and focus. Shrinking
 * gives keys left in the row or the content to the current terminal, as when another Tab replaces
 * the terminal behind the overlay; an ask for the keys while the overlay stays shrunk gives them to
 * the shield.
 */
export function FileOverlayPip({
	box,
	contentReference,
	frameReference,
	move,
	regions,
}: FileOverlayPipProps): ReactNode {
	const intl = useOrchbeeIntl();
	const dispatch = useFileOverlayHostDispatch();
	const focusRequest = useFileOverlayHostFocusRequest();
	const focusTerminal = useFileOverlayHostFocusTerminal();
	const returnFocus = useMemoizedFn(() => focusTerminal());
	const shieldReference = useRef<HTMLButtonElement>(null);
	const drag = useFileOverlayPipDrag({ box, frameReference, move, regions });
	// The row hides and the content goes out of reach as picture-in-picture arrives.
	useLayoutEffect(() => {
		const frame = frameReference.current;
		if (frame !== null && frame.contains(document.activeElement)) returnFocus();
	}, [frameReference, returnFocus]);
	useLayoutEffect(() => {
		const shield = shieldReference.current;
		if (focusRequest.count === 0 || shield === null || !focusRequest.take()) return;
		shield.focus({ preventScroll: true });
	}, [focusRequest]);
	const restore = useMemoizedFn(() => {
		if (drag.moved()) return;
		dispatch({ type: 'pip-restored' });
		// The shield goes as the overlay shows, so the active tab takes the keys.
		focusRequest.ask();
	});
	const forwardWheel = useMemoizedFn((event: WheelEvent<HTMLButtonElement>) => {
		const content = contentReference.current;
		if (content !== null) forwardFileOverlayPipWheel(content, event.nativeEvent);
	});
	return (
		<div className={styles['root']} data-slot='file-overlay-pip'>
			<button
				aria-label={intl.formatMessage(fileOverlayMessages.restorePip)}
				className={styles['shield']}
				type='button'
				onClick={restore}
				onPointerDown={drag.onPointerDown}
				onWheel={forwardWheel}
				ref={shieldReference}
			/>
			<FileOverlayPipClose />
		</div>
	);
}

/** Picture-in-picture's own close control, a second button beside the shield that closes every tab. */
function FileOverlayPipClose(): ReactNode {
	const intl = useOrchbeeIntl();
	const commands = useFileOverlayHostCommands();
	const shortcutLabels = useFileOverlayHostShortcutLabels();
	const label = intl.formatMessage(fileOverlayMessages.closeOverlay);
	return (
		<Tooltip>
			<TooltipTrigger
				render={
					<Button
						aria-label={label}
						className={styles['close']}
						data-slot='file-overlay-pip-close'
						size='icon-sm'
						variant='secondary'
						onClick={commands.closeOverlay}
					/>
				}
			>
				<ProductIcon name='close' />
			</TooltipTrigger>
			<TooltipContent align='end' side='bottom'>
				{label}
				{shortcutLabels.closeOverlay === undefined ? null : (
					<Kbd>{shortcutLabels.closeOverlay}</Kbd>
				)}
			</TooltipContent>
		</Tooltip>
	);
}
