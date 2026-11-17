import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import { useTerminalSettings } from '../../terminal-settings';
import { TERMINAL_SETTINGS_CATEGORIES } from '../constants/terminal-settings-categories';
import type {
	SettingsChildPageEntry,
	SettingsSurfaceSlots,
} from '../contexts/settings-surface-slots';
import type {
	TerminalSettingsCategory,
	TerminalSettingsSurfaceProps,
} from '../types/terminal-settings-surface';

interface TerminalSettingsSurfaceState {
	readonly categories: typeof TERMINAL_SETTINGS_CATEGORIES;
	readonly category: TerminalSettingsCategory;
	/** A child page covers the category's heading and content. */
	readonly childPageOpen: boolean;
	readonly childPageReference: (element: HTMLElement | null) => void;
	readonly contentReference: React.RefObject<HTMLElement | null>;
	readonly dismissBanner: (id: string) => void;
	readonly isBannerDismissed: (id: string) => boolean;
	readonly noticeReference: (element: HTMLElement | null) => void;
	readonly onCategorySelect: (category: TerminalSettingsCategory) => void;
	readonly onQueryChange: (query: string) => void;
	readonly onSurfaceKeyDown: React.KeyboardEventHandler<HTMLDivElement>;
	readonly open: boolean;
	readonly query: string;
	readonly searchReference: React.RefObject<HTMLInputElement | null>;
	readonly searching: boolean;
	readonly slots: SettingsSurfaceSlots;
}

/** An open child page, and where leaving it returns: the scroll offset and the control it came from. */
interface OpenChildPage {
	readonly entry: SettingsChildPageEntry;
	readonly opener: Element | null;
	readonly scrollTop: number;
}

/**
 * Page state for the Settings dialog: the open category, the search query, dismissed banners, and
 * the child page an action opened. Escape leaves a child page first, then clears a query, and
 * otherwise returns to where the user was.
 */
export function useTerminalSettingsSurface({
	about,
	agentIntegrationManagementControl,
	controller,
	initialCategory,
	macosTerminalInputSettingsController,
	onRequestClose,
	terminalDefaultSettingsControl,
	terminalFeedbackSettingsController,
	terminalInteractionSettingsController,
	terminalLaunchProfileClient,
}: TerminalSettingsSurfaceProps): TerminalSettingsSurfaceState {
	const snapshot = useTerminalSettings(controller);
	const [category, setCategory] = React.useState<TerminalSettingsCategory>(
		initialCategory ?? 'general',
	);
	const [query, setQuery] = React.useState('');
	const [dismissed, setDismissed] = React.useState<ReadonlySet<string>>(() => new Set());
	const searchReference = React.useRef<HTMLInputElement>(null);
	const contentReference = React.useRef<HTMLElement>(null);
	const [childPageHost, setChildPageHost] = React.useState<HTMLElement | null>(null);
	const [noticeHost, setNoticeHost] = React.useState<HTMLElement | null>(null);
	const [childPage, setChildPage] = React.useState<OpenChildPage | null>(null);
	const returning = React.useRef<OpenChildPage | null>(null);

	const enterChildPage = useMemoizedFn((entry: SettingsChildPageEntry): (() => void) => {
		const content = contentReference.current;
		const opened: OpenChildPage = {
			entry,
			opener: content?.ownerDocument.activeElement ?? null,
			scrollTop: content?.scrollTop ?? 0,
		};
		setChildPage(opened);
		return () => {
			returning.current = opened;
			setChildPage((current) => (current === opened ? null : current));
		};
	});
	const slots = React.useMemo<SettingsSurfaceSlots>(
		() => ({ childPageHost, enterChildPage, noticeHost }),
		[childPageHost, enterChildPage, noticeHost],
	);

	// A child page opens at its top; going back puts back the list's scroll offset and focus. A page
	// left by switching category or searching has no opener on screen and keeps the new view's top.
	React.useLayoutEffect(() => {
		const content = contentReference.current;
		if (childPage !== null) {
			if (content !== null) content.scrollTop = 0;
			return;
		}
		const left = returning.current;
		returning.current = null;
		if (left === null || !(left.opener instanceof HTMLElement) || !left.opener.isConnected) return;
		if (content !== null) content.scrollTop = left.scrollTop;
		left.opener.focus({ preventScroll: true });
	}, [childPage]);

	const terminalAvailable =
		terminalInteractionSettingsController !== undefined ||
		terminalDefaultSettingsControl !== undefined ||
		terminalLaunchProfileClient !== undefined ||
		macosTerminalInputSettingsController !== undefined;
	const categories = TERMINAL_SETTINGS_CATEGORIES.filter(
		(definition) =>
			(definition.id !== 'terminal' || terminalAvailable) &&
			(definition.id !== 'notifications' || terminalFeedbackSettingsController !== undefined) &&
			(definition.id !== 'about' || about !== undefined),
	);

	React.useEffect(() => {
		if (!snapshot.open) return;
		if (snapshot.landing === 'keyboard-shortcuts') setCategory('keyboard');
		setQuery('');
		searchReference.current?.focus();
	}, [snapshot.landing, snapshot.open]);

	// Integrations are opt-in: entering their category is the explicit request to inspect them, so
	// opening Settings alone runs no provider command.
	const onCategorySelect = (nextCategory: TerminalSettingsCategory): void => {
		if (
			nextCategory === 'agents' &&
			category !== 'agents' &&
			agentIntegrationManagementControl !== undefined &&
			!agentIntegrationManagementControl.getSnapshot().pending
		) {
			void agentIntegrationManagementControl.refresh();
		}
		setCategory(nextCategory);
		setQuery('');
		if (contentReference.current !== null) contentReference.current.scrollTop = 0;
	};

	return {
		categories,
		category: categories.some((definition) => definition.id === category) ? category : 'general',
		childPageOpen: childPage !== null,
		childPageReference: setChildPageHost,
		contentReference,
		dismissBanner: (id) => setDismissed((current) => new Set([...current, id])),
		isBannerDismissed: (id) => dismissed.has(id),
		noticeReference: setNoticeHost,
		onCategorySelect,
		onQueryChange: setQuery,
		onSurfaceKeyDown: (event) => {
			if (event.key !== 'Escape' || event.defaultPrevented) return;
			event.preventDefault();
			if (childPage !== null) {
				childPage.entry.back();
				return;
			}
			if (query.length > 0) {
				setQuery('');
				searchReference.current?.focus();
				return;
			}
			onRequestClose();
		},
		open: snapshot.open,
		query,
		searchReference,
		searching: query.trim().length > 0,
		slots,
	};
}
