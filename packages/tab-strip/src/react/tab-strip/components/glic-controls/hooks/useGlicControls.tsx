import {
	useEffect,
	useRef,
	useState,
	useSyncExternalStore,
	type MouseEvent,
	type PointerEvent,
	type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { GlicButtonWidthState, getGlicButtonPresentation } from '../../../../../glic';
import type { GlicControlsController } from '../../../../../glic';
import { TabStripIcon } from '../../tab-strip-icon/TabStripIcon';
interface UseGlicControlsOptions {
	controller: GlicControlsController;
	onGlicClick?: () => void;
	onActorClick?: () => void;
	onUnpin?: () => void;
	activeTabId?: string | null;
	className?: string;
}
export function useGlicControls({
	controller,
	onGlicClick,
	onActorClick,
	onUnpin,
	activeTabId,
	className = '',
}: UseGlicControlsOptions): ReactNode {
	const snapshot = useSyncExternalStore(
		controller.subscribe,
		controller.getSnapshot,
		controller.getSnapshot,
	);
	const [menu, setMenu] = useState<{
		x: number;
		y: number;
	} | null>(null);
	const previousActiveTabId = useRef(activeTabId);
	const presentation = getGlicButtonPresentation(snapshot.widthState, snapshot.nudgeLabel);
	useEffect(() => {
		if (!menu) return;
		const close = () => setMenu(null);
		window.addEventListener('pointerdown', close);
		window.addEventListener('blur', close);
		return () => {
			window.removeEventListener('pointerdown', close);
			window.removeEventListener('blur', close);
		};
	}, [menu]);
	useEffect(() => {
		if (previousActiveTabId.current !== activeTabId) controller.notifyActiveTabChanged();
		previousActiveTabId.current = activeTabId;
	}, [activeTabId, controller]);
	return (
		<div
			className={`ct-glic-controls ${className}`.trim()}
			role='group'
			aria-label='Gemini and actor controls'
			data-highlighted={snapshot.panelOpen || snapshot.actorPressed || undefined}
			data-split={snapshot.actorVisible || undefined}
			data-locked-expansion={snapshot.lockedExpansionMode}
			onMouseEnter={() => controller.setLockedExpansionHovered(true)}
			onMouseLeave={() => controller.setLockedExpansionHovered(false)}
		>
			<div className='ct-glic-controls__gemini' data-nudge-transition={snapshot.nudgeTransition}>
				<button
					type='button'
					className='ct-glic-button'
					aria-label={snapshot.panelOpen ? 'Close Gemini' : 'Open Gemini'}
					title={snapshot.panelOpen ? 'Close Gemini' : 'Open Gemini'}
					data-collapsed={!presentation.showLabel || undefined}
					onClick={() => {
						controller.setPanelOpen(!snapshot.panelOpen);
						if (snapshot.widthState === GlicButtonWidthState.Nudge) controller.dismissGlicNudge();
						onGlicClick?.();
					}}
					onContextMenu={(event: MouseEvent<HTMLButtonElement>) => {
						if (!snapshot.pinned) return;
						event.preventDefault();
						setMenu({
							x: event.clientX,
							y: event.clientY,
						});
					}}
				>
					<TabStripIcon name='glic-spark' />
					{presentation.showLabel ? <span>{presentation.label}</span> : null}
				</button>
				{presentation.showCloseButton ? (
					<button
						type='button'
						className='ct-glic-button__close'
						aria-label='Dismiss Gemini suggestion'
						onClick={() => controller.dismissGlicNudge()}
					>
						{'\xD7'}
					</button>
				) : null}
			</div>
			{snapshot.actorVisible ? (
				<button
					type='button'
					className='ct-glic-actor'
					aria-label={snapshot.actorNudgeShowing ? snapshot.actorNudgeLabel : 'Actor tasks'}
					aria-pressed={snapshot.actorPressed}
					onClick={() => {
						controller.setActorPressed(!snapshot.actorPressed);
						onActorClick?.();
					}}
				>
					<TabStripIcon name='glic-actor' />
					{snapshot.actorNudgeShowing && snapshot.actorNudgeLabel ? (
						<span>{snapshot.actorNudgeLabel}</span>
					) : null}
				</button>
			) : null}
			{menu
				? createPortal(
						<div
							className='ct-menu ct-glic-menu'
							role='menu'
							style={{
								left: menu.x,
								top: menu.y,
							}}
							onPointerDown={(event: PointerEvent<HTMLDivElement>) => event.stopPropagation()}
						>
							<button
								role='menuitem'
								onClick={() => {
									controller.setPinned(false);
									setMenu(null);
									onUnpin?.();
								}}
							>
								{'Unpin Gemini'}
							</button>
						</div>,
						document.body,
					)
				: null}
		</div>
	);
}
