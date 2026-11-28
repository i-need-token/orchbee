import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileOverlayMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { ProductIcon, Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import { useLayoutEffect, useRef, type MouseEvent, type PointerEvent, type ReactNode } from 'react';

import {
	useFileOverlayHostCommands,
	useFileOverlayHostPlatform,
} from '../../../../../../contexts/file-overlay-host-store';
import styles from './FileTab.module.css';

interface FileTabProps {
	readonly active: boolean;
	/** The folders that tell the tab from another showing the same name; empty when none do. */
	readonly description: string;
	readonly domId: string;
	readonly name: string;
	/** The content panel's element id, which the active tab controls. */
	readonly panelId: string;
	/** The file's whole path, which the tooltip shows. */
	readonly path: string;
	readonly preview: boolean;
	readonly sticky: boolean;
	readonly tabId: string;
	readonly unsaved: boolean;
	/** Called as the tab leaves the row while it holds the focus. */
	readonly onLeaveWithFocus: () => void;
}

/** Whether a press began on the tab's own button, which handles it instead of the tab. */
function pressedAction(target: EventTarget): boolean {
	return target instanceof Element && target.closest('[data-slot="file-tab-action"]') !== null;
}

/**
 * One file tab, which behaves as VS Code's tabs do. A press shows it at once, except a Control-press
 * on a Mac, which asks for its menu, and a touch, which shows it on release so that a swipe can
 * scroll the row. A middle click closes it unless it is sticky, and a double-click keeps a preview
 * tab. Its button closes it, or unpins a sticky tab; the dot of unsaved edits stands in the button's
 * place until the pointer reaches the button.
 */
export function FileTab({
	active,
	description,
	domId,
	name,
	panelId,
	path,
	preview,
	sticky,
	tabId,
	unsaved,
	onLeaveWithFocus,
}: FileTabProps): ReactNode {
	const intl = useOrchbeeIntl();
	const commands = useFileOverlayHostCommands();
	const platform = useFileOverlayHostPlatform();
	const tabReference = useRef<HTMLDivElement>(null);
	const pointerType = useRef('mouse');
	useLayoutEffect(() => {
		const tab = tabReference.current;
		// A leaving tab is still in the document while its layout Effects clean up.
		return () => {
			if (tab !== null && tab.ownerDocument.activeElement === tab) onLeaveWithFocus();
		};
	}, [onLeaveWithFocus]);
	const onPointerDown = useMemoizedFn((event: PointerEvent<HTMLDivElement>) => {
		pointerType.current = event.pointerType;
		if (pressedAction(event.target)) return;
		// A middle press would start the browser's autoscroll; its click closes the tab instead.
		if (event.button === 1) event.preventDefault();
		if (event.button !== 0 || event.pointerType === 'touch') return;
		if (platform === 'apple' && event.ctrlKey) return;
		commands.selectTab(tabId, 'pointer');
	});
	const onClick = useMemoizedFn((event: MouseEvent<HTMLDivElement>) => {
		if (pointerType.current === 'touch' && !pressedAction(event.target)) {
			commands.selectTab(tabId, 'pointer');
		}
	});
	// VS Code keeps a sticky tab from a middle click by default.
	const onAuxClick = useMemoizedFn((event: MouseEvent<HTMLDivElement>) => {
		if (event.button === 1 && !sticky) commands.closeTab(tabId, 'pointer');
	});
	const onDoubleClick = useMemoizedFn((event: MouseEvent<HTMLDivElement>) => {
		if (preview && !pressedAction(event.target)) commands.keepTab(tabId, 'pointer');
	});
	const onAction = useMemoizedFn(() => {
		if (sticky) commands.unpinTab(tabId, 'button');
		else commands.closeTab(tabId, 'button');
	});
	const label = description === '' ? name : `${name} ${description}`;
	let accessibleName = label;
	// VS Code names a tab's state after its name; a preview tab is never sticky.
	if (preview) {
		accessibleName = intl.formatMessage(fileOverlayMessages.previewTabName, { name: label });
	} else if (sticky) {
		accessibleName = intl.formatMessage(fileOverlayMessages.pinnedTabName, { name: label });
	}
	return (
		<Tooltip>
			{/* The tab's id goes to the trigger, which Base UI tracks by its id: given only to the element it
			    renders, the id would replace the trigger's own and the tooltip would never open. */}
			<TooltipTrigger
				id={domId}
				render={
					<div
						aria-controls={active ? panelId : undefined}
						aria-label={accessibleName}
						aria-selected={active}
						className={styles['tab']}
						data-active={active ? '' : undefined}
						data-preview={preview ? '' : undefined}
						data-slot='file-tab'
						data-sticky={sticky ? '' : undefined}
						data-unsaved={unsaved ? '' : undefined}
						ref={tabReference}
						role='tab'
						tabIndex={active ? 0 : -1}
						onAuxClick={onAuxClick}
						onClick={onClick}
						onDoubleClick={onDoubleClick}
						onPointerDown={onPointerDown}
					/>
				}
			>
				<span className={styles['icon']}>
					<ProductIcon name='file' />
				</span>
				<span className={styles['label']}>
					<span>{name}</span>
					{description === '' ? null : <span className={styles['description']}>{description}</span>}
				</span>
				<FileTabAction
					label={intl.formatMessage(sticky ? fileOverlayMessages.unpin : fileOverlayMessages.close)}
					sticky={sticky}
					unsaved={unsaved}
					onPress={onAction}
				/>
			</TooltipTrigger>
			<TooltipContent align='start' className={styles['path']} side='bottom'>
				{preview ? intl.formatMessage(fileOverlayMessages.previewTabPath, { path }) : path}
			</TooltipContent>
		</Tooltip>
	);
}

interface FileTabActionProps {
	readonly label: string;
	readonly sticky: boolean;
	readonly unsaved: boolean;
	readonly onPress: () => void;
}

/**
 * The tab's close button, or its unpin button while it is sticky. It never takes the focus, as the
 * Tab Strip's close button does not, so a press leaves the keys where they were.
 */
function FileTabAction({ label, sticky, unsaved, onPress }: FileTabActionProps): ReactNode {
	const glyph = sticky ? 'pin' : 'close';
	return (
		<button
			aria-label={label}
			className={styles['action']}
			data-slot='file-tab-action'
			tabIndex={-1}
			type='button'
			onClick={onPress}
			onPointerDown={(event) => event.preventDefault()}
		>
			{unsaved ? (
				<>
					<span className={styles['restGlyph']}>
						<ProductIcon name={sticky ? 'pin-unsaved' : 'unsaved'} />
					</span>
					<span className={styles['hoverGlyph']}>
						<ProductIcon name={glyph} />
					</span>
				</>
			) : (
				<ProductIcon name={glyph} />
			)}
		</button>
	);
}
