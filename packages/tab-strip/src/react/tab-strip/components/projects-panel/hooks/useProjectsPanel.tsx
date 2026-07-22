import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import type { TabGroupColor } from '../../../../../model';
import {
	PROJECTS_PANEL_COLLAPSED_THREAD_COUNT,
	PROJECTS_PANEL_CORNER_RADIUS,
	PROJECTS_PANEL_HIDE_DURATION_MS,
	PROJECTS_PANEL_MAX_RECENT_THREADS,
	PROJECTS_PANEL_MIN_WIDTH,
	PROJECTS_PANEL_SHOW_DURATION_MS,
} from '../../../../../projects';
import type { ProjectsPanelController } from '../../../../../projects';
const PROJECT_COLOR: Record<TabGroupColor, string> = {
	grey: '#5f6368',
	blue: '#1a73e8',
	red: '#d93025',
	yellow: '#f9ab00',
	green: '#188038',
	pink: '#d01884',
	purple: '#9334e6',
	cyan: '#007b83',
	orange: '#fa7b17',
};
type ProjectsPanelCloseReason = 'button' | 'escape' | 'outside-pointer' | 'focus-lost';
interface UseProjectsPanelOptions {
	controller: ProjectsPanelController;
	open: boolean;
	onClose: (reason: ProjectsPanelCloseReason) => void;
	targetWidth?: number;
	elevated?: boolean;
	title?: string;
	className?: string;
}
export function useProjectsPanel({
	controller,
	open,
	onClose,
	targetWidth = PROJECTS_PANEL_MIN_WIDTH,
	elevated = true,
	title = 'Projects',
	className = '',
}: UseProjectsPanelOptions): ReactNode {
	const snapshot = useSyncExternalStore(
		controller.subscribe,
		controller.getSnapshot,
		controller.getSnapshot,
	);
	const [mounted, setMounted] = useState(open);
	const [threadsExpanded, setThreadsExpanded] = useState(false);
	const rootRef = useRef<HTMLElement>(null);
	const previousFocusRef = useRef<HTMLElement | null>(null);
	const closingRef = useRef(false);
	useEffect(() => {
		if (open) {
			setMounted(true);
			closingRef.current = false;
			return;
		}
		const timer = window.setTimeout(() => setMounted(false), PROJECTS_PANEL_HIDE_DURATION_MS);
		return () => window.clearTimeout(timer);
	}, [open]);
	useEffect(() => {
		if (!open) return;
		previousFocusRef.current =
			document.activeElement instanceof HTMLElement ? document.activeElement : null;
		rootRef.current?.focus();
		const closeOnce = (reason: ProjectsPanelCloseReason) => {
			if (closingRef.current) return;
			closingRef.current = true;
			onClose(reason);
		};
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.key !== 'Escape') return;
			event.preventDefault();
			closeOnce('escape');
		};
		const onPointerDown = (event: PointerEvent) => {
			if (event.target instanceof Node && !rootRef.current?.contains(event.target))
				closeOnce('outside-pointer');
		};
		const onFocusIn = (event: FocusEvent) => {
			if (event.target instanceof Node && !rootRef.current?.contains(event.target))
				closeOnce('focus-lost');
		};
		window.addEventListener('keydown', onKeyDown);
		window.addEventListener('pointerdown', onPointerDown, true);
		window.addEventListener('focusin', onFocusIn);
		return () => {
			window.removeEventListener('keydown', onKeyDown);
			window.removeEventListener('pointerdown', onPointerDown, true);
			window.removeEventListener('focusin', onFocusIn);
			previousFocusRef.current?.focus();
			previousFocusRef.current = null;
		};
	}, [onClose, open]);
	if (!mounted) return null;
	const recentThreads = snapshot.threads.slice(0, PROJECTS_PANEL_MAX_RECENT_THREADS);
	const visibleThreads = threadsExpanded
		? recentThreads
		: recentThreads.slice(0, PROJECTS_PANEL_COLLAPSED_THREAD_COUNT);
	const width = Math.max(PROJECTS_PANEL_MIN_WIDTH, targetWidth);
	const isMac = typeof navigator !== 'undefined' && /Mac/.test(navigator.platform);
	return (
		<aside
			ref={rootRef}
			className={`ct-projects-panel ${className}`.trim()}
			role='region'
			aria-label={title}
			tabIndex={-1}
			data-open={open || undefined}
			data-elevated={elevated || undefined}
			data-mac={isMac || undefined}
			style={
				{
					'--ct-projects-target-width': `${width}px`,
					'--ct-projects-corner-radius': elevated ? `${PROJECTS_PANEL_CORNER_RADIUS}px` : '0px',
					'--ct-projects-duration': `${open ? PROJECTS_PANEL_SHOW_DURATION_MS : PROJECTS_PANEL_HIDE_DURATION_MS}ms`,
				} satisfies CSSProperties &
					Record<
						'--ct-projects-target-width' | '--ct-projects-corner-radius' | '--ct-projects-duration',
						string
					>
			}
		>
			<div className='ct-projects-panel__surface'>
				<header className='ct-projects-panel__controls'>
					<strong>{title}</strong>
					<button
						type='button'
						aria-label='Close Projects'
						title='Close Projects'
						onClick={() => onClose('button')}
					>
						{'\xD7'}
					</button>
				</header>

				<div className='ct-projects-panel__content'>
					<section aria-labelledby='ct-projects-groups-heading'>
						<h2 id='ct-projects-groups-heading'>{'Tab groups'}</h2>
						{snapshot.tabGroups.length ? (
							<ul className='ct-projects-list'>
								{snapshot.tabGroups.map((group, index) => (
									<li key={group.id}>
										<button
											type='button'
											className='ct-projects-list__main'
											aria-label={`Open ${group.title || 'unnamed group'}${group.tabCount === undefined ? '' : `, ${group.tabCount} ${group.tabCount === 1 ? 'tab' : 'tabs'}`}`}
											onClick={() => controller.openTabGroup(group.id)}
										>
											<span
												className='ct-projects-list__group-icon'
												style={
													{
														'--ct-project-color': PROJECT_COLOR[group.color],
													} satisfies CSSProperties & Record<'--ct-project-color', string>
												}
											/>
											<span>{group.title || 'Unnamed group'}</span>
											{group.tabCount !== undefined ? (
												<small>
													{group.tabCount} {group.tabCount === 1 ? 'tab' : 'tabs'}
												</small>
											) : null}
										</button>

										<span className='ct-projects-list__moves'>
											<button
												type='button'
												aria-label={`Move ${group.title || 'group'} up`}
												disabled={index === 0}
												onClick={() => controller.moveTabGroup(group.id, index - 1)}
											>
												{'\u2191'}
											</button>

											<button
												type='button'
												aria-label={`Move ${group.title || 'group'} down`}
												disabled={index === snapshot.tabGroups.length - 1}
												onClick={() => controller.moveTabGroup(group.id, index + 1)}
											>
												{'\u2193'}
											</button>
										</span>
									</li>
								))}
							</ul>
						) : (
							<p className='ct-projects-panel__empty'>{'No saved tab groups'}</p>
						)}
					</section>

					<div className='ct-projects-panel__separator' />
					<section aria-labelledby='ct-projects-threads-heading'>
						<h2 id='ct-projects-threads-heading'>{'Recent threads'}</h2>
						{visibleThreads.length ? (
							<ul className='ct-projects-list ct-projects-list--threads'>
								{visibleThreads.map((thread) => (
									<li key={thread.serverId}>
										<button
											type='button'
											className='ct-projects-list__main'
											onClick={() => controller.openThread(thread.serverId)}
										>
											<span className='ct-projects-list__thread-icon' aria-hidden='true'>
												{thread.type === 'gemini' ? '✦' : '⌕'}
											</span>
											<span>{thread.title || 'Untitled thread'}</span>
										</button>
									</li>
								))}
							</ul>
						) : (
							<p className='ct-projects-panel__empty'>{'No recent threads'}</p>
						)}
						{recentThreads.length > PROJECTS_PANEL_COLLAPSED_THREAD_COUNT ? (
							<button
								type='button'
								className='ct-projects-panel__show-more'
								aria-expanded={threadsExpanded}
								onClick={() => setThreadsExpanded((value) => !value)}
							>
								{threadsExpanded
									? 'Show less'
									: `Show ${recentThreads.length - PROJECTS_PANEL_COLLAPSED_THREAD_COUNT} more`}
							</button>
						) : null}
					</section>
				</div>
			</div>
		</aside>
	);
}
