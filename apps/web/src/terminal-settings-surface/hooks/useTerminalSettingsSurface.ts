import * as React from 'react';

import { useTerminalSettings } from '../../terminal-settings';
import { TERMINAL_SETTINGS_CATEGORIES } from '../constants/terminal-settings-categories';
import type {
	TerminalSettingsCategory,
	TerminalSettingsSurfaceProps,
} from '../types/terminal-settings-surface';

interface TerminalSettingsSurfaceState {
	readonly categories: typeof TERMINAL_SETTINGS_CATEGORIES;
	readonly category: TerminalSettingsCategory;
	readonly contentReference: React.RefObject<HTMLElement | null>;
	readonly dismissBanner: (id: string) => void;
	readonly isBannerDismissed: (id: string) => boolean;
	readonly onCategorySelect: (category: TerminalSettingsCategory) => void;
	readonly onQueryChange: (query: string) => void;
	readonly onSurfaceKeyDown: React.KeyboardEventHandler<HTMLDivElement>;
	readonly open: boolean;
	readonly query: string;
	readonly searchReference: React.RefObject<HTMLInputElement | null>;
	readonly searching: boolean;
}

/**
 * Page state for full-page Settings: the open category, the search query, and dismissed banners.
 * Escape clears a query first and otherwise returns to where the user was.
 */
export function useTerminalSettingsSurface({
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

	const terminalAvailable =
		terminalInteractionSettingsController !== undefined ||
		terminalDefaultSettingsControl !== undefined ||
		terminalLaunchProfileClient !== undefined ||
		macosTerminalInputSettingsController !== undefined;
	const categories = TERMINAL_SETTINGS_CATEGORIES.filter(
		(definition) =>
			(definition.id !== 'terminal' || terminalAvailable) &&
			(definition.id !== 'notifications' || terminalFeedbackSettingsController !== undefined),
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
		contentReference,
		dismissBanner: (id) => setDismissed((current) => new Set([...current, id])),
		isBannerDismissed: (id) => dismissed.has(id),
		onCategorySelect,
		onQueryChange: setQuery,
		onSurfaceKeyDown: (event) => {
			if (event.key !== 'Escape' || event.defaultPrevented) return;
			event.preventDefault();
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
	};
}
