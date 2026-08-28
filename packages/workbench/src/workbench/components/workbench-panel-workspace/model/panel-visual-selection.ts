import { collectPanelLeaves, type PanelLayoutNode } from './panel-workspace-state';

const PANEL_VISUAL_SELECTION_ALPHABET = '1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ';

export type PanelVisualSelectionMode = 'focus' | 'swap';

interface PanelVisualSelectionTarget {
	readonly label: string;
	readonly panelId: string;
}

export interface PanelVisualSelectionSession {
	readonly mode: PanelVisualSelectionMode;
	readonly panelIds: readonly string[];
	readonly prefix: string;
	readonly sourcePanelId: string;
	readonly tabId: string;
	readonly targets: readonly PanelVisualSelectionTarget[];
}

export type PanelVisualSelectionCommand =
	| {
			readonly kind: 'focus';
			readonly sourcePanelId: string;
			readonly tabId: string;
			readonly targetPanelId: string;
	  }
	| {
			readonly kind: 'swap';
			readonly sourcePanelId: string;
			readonly tabId: string;
			readonly targetPanelId: string;
	  };

interface StartPanelVisualSelectionOptions {
	readonly maximizedPanelId: string | null;
	readonly mode: PanelVisualSelectionMode;
	readonly root: PanelLayoutNode;
	readonly sourcePanelId: string;
	readonly tabId: string;
}

export type PanelVisualSelectionStartResult =
	| {
			readonly kind: 'unavailable';
			readonly reason:
				| 'invalid-panel-order'
				| 'panel-maximized'
				| 'requires-multiple-panels'
				| 'source-panel-unavailable';
	  }
	| {
			readonly command: PanelVisualSelectionCommand;
			readonly kind: 'direct';
	  }
	| {
			readonly kind: 'started';
			readonly session: PanelVisualSelectionSession;
	  };

export type PanelVisualSelectionEvent =
	| {
			readonly code: string;
			readonly isComposing: boolean;
			readonly repeat: boolean;
			readonly type: 'key-pressed';
	  }
	| {
			readonly panelId: string;
			readonly type: 'target-selected';
	  }
	| {
			readonly restoreSourceFocus: boolean;
			readonly type: 'cancelled';
	  }
	| {
			readonly activeTabId: string | null;
			readonly focusedPanelId: string | null;
			readonly maximizedPanelId: string | null;
			readonly root: PanelLayoutNode | null;
			readonly type: 'workspace-revalidated';
	  };

export type PanelVisualSelectionTransition =
	| {
			readonly kind: 'active';
			readonly outcome: 'unchanged' | 'updated';
			readonly session: PanelVisualSelectionSession;
	  }
	| {
			readonly command: PanelVisualSelectionCommand | null;
			readonly kind: 'closed';
			readonly outcome: 'cancelled' | 'selected' | 'stale';
			readonly restorePanelId: string | null;
	  };

/** Resolve one exact direct target or create a bounded presenter-owned selection session. */
export function startPanelVisualSelection({
	maximizedPanelId,
	mode,
	root,
	sourcePanelId,
	tabId,
}: StartPanelVisualSelectionOptions): PanelVisualSelectionStartResult {
	if (maximizedPanelId !== null) return { kind: 'unavailable', reason: 'panel-maximized' };

	const panelIds = collectPanelLeaves(root).map((panel) => panel.id);
	if (panelIds.length < 2) {
		return { kind: 'unavailable', reason: 'requires-multiple-panels' };
	}
	if (!panelIds.includes(sourcePanelId)) {
		return { kind: 'unavailable', reason: 'source-panel-unavailable' };
	}
	if (new Set(panelIds).size !== panelIds.length) {
		return { kind: 'unavailable', reason: 'invalid-panel-order' };
	}

	const targetPanelIds = panelIds.filter((panelId) => panelId !== sourcePanelId);
	const directTargetPanelId = targetPanelIds[0];
	if (targetPanelIds.length === 1 && directTargetPanelId !== undefined) {
		return {
			command: commandFor(mode, tabId, sourcePanelId, directTargetPanelId),
			kind: 'direct',
		};
	}

	const labels = createPanelVisualSelectionLabels(targetPanelIds.length);
	const targets = Object.freeze(
		targetPanelIds.map((panelId, index) => Object.freeze({ label: labels[index] ?? '', panelId })),
	);
	return {
		kind: 'started',
		session: Object.freeze({
			mode,
			panelIds: Object.freeze([...panelIds]),
			prefix: '',
			sourcePanelId,
			tabId,
			targets,
		}),
	};
}

/** Advance one active selection without writing the authoritative Panel workspace. */
export function transitionPanelVisualSelection(
	session: PanelVisualSelectionSession,
	event: PanelVisualSelectionEvent,
): PanelVisualSelectionTransition {
	switch (event.type) {
		case 'cancelled':
			return closed('cancelled', null, event.restoreSourceFocus ? session.sourcePanelId : null);
		case 'target-selected':
			return selectTarget(session, event.panelId);
		case 'workspace-revalidated': {
			const currentPanelIds =
				event.root === null ? [] : collectPanelLeaves(event.root).map((panel) => panel.id);
			return event.activeTabId === session.tabId &&
				event.focusedPanelId === session.sourcePanelId &&
				event.maximizedPanelId === null &&
				equalOrder(currentPanelIds, session.panelIds)
				? active(session, 'unchanged')
				: closed('stale', null, null);
		}
		case 'key-pressed': {
			if (event.isComposing || event.repeat) return active(session, 'unchanged');
			if (event.code === 'Escape') {
				return closed('cancelled', null, session.sourcePanelId);
			}
			if (event.code === 'Backspace') {
				if (session.prefix.length === 0) return active(session, 'unchanged');
				return active(withPrefix(session, session.prefix.slice(0, -1)), 'updated');
			}

			const character = characterForCode(event.code);
			if (character === null) return active(session, 'unchanged');
			const nextPrefix = `${session.prefix}${character}`;
			const matches = session.targets.filter((target) => target.label.startsWith(nextPrefix));
			if (matches.length === 0) return active(session, 'unchanged');
			const selected = matches.find((target) => target.label === nextPrefix);
			return selected === undefined
				? active(withPrefix(session, nextPrefix), 'updated')
				: selectTarget(session, selected.panelId);
		}
	}
}

function createPanelVisualSelectionLabels(count: number): readonly string[] {
	if (!Number.isSafeInteger(count) || count < 0) {
		throw new RangeError('Panel visual selection label count must be a non-negative safe integer.');
	}
	if (count === 0) return [];

	let width = 1;
	let capacity = PANEL_VISUAL_SELECTION_ALPHABET.length;
	while (capacity < count) {
		width += 1;
		capacity *= PANEL_VISUAL_SELECTION_ALPHABET.length;
	}
	return Object.freeze(Array.from({ length: count }, (_value, index) => encodeLabel(index, width)));
}

function active(
	session: PanelVisualSelectionSession,
	outcome: 'unchanged' | 'updated',
): PanelVisualSelectionTransition {
	return { kind: 'active', outcome, session };
}

function characterForCode(code: string): string | null {
	if (code.length === 6 && code.startsWith('Digit')) {
		const character = code.slice(5);
		return character >= '0' && character <= '9' ? character : null;
	}
	if (code.length === 4 && code.startsWith('Key')) {
		const character = code.slice(3);
		return character >= 'A' && character <= 'Z' ? character : null;
	}
	return null;
}

function closed(
	outcome: 'cancelled' | 'selected' | 'stale',
	command: PanelVisualSelectionCommand | null,
	restorePanelId: string | null,
): PanelVisualSelectionTransition {
	return { command, kind: 'closed', outcome, restorePanelId };
}

function commandFor(
	mode: PanelVisualSelectionMode,
	tabId: string,
	sourcePanelId: string,
	targetPanelId: string,
): PanelVisualSelectionCommand {
	return { kind: mode, sourcePanelId, tabId, targetPanelId };
}

function encodeLabel(index: number, width: number): string {
	const encoded = Array.from({ length: width }, () => PANEL_VISUAL_SELECTION_ALPHABET[0] ?? '1');
	let remaining = index;
	for (let position = width - 1; position >= 0; position -= 1) {
		encoded[position] =
			PANEL_VISUAL_SELECTION_ALPHABET[remaining % PANEL_VISUAL_SELECTION_ALPHABET.length] ?? '1';
		remaining = Math.floor(remaining / PANEL_VISUAL_SELECTION_ALPHABET.length);
	}
	return encoded.join('');
}

function equalOrder(left: readonly string[], right: readonly string[]): boolean {
	return left.length === right.length && left.every((panelId, index) => panelId === right[index]);
}

function selectTarget(
	session: PanelVisualSelectionSession,
	targetPanelId: string,
): PanelVisualSelectionTransition {
	if (!session.targets.some((target) => target.panelId === targetPanelId)) {
		return closed('stale', null, null);
	}
	return closed(
		'selected',
		commandFor(session.mode, session.tabId, session.sourcePanelId, targetPanelId),
		session.mode === 'focus' ? targetPanelId : session.sourcePanelId,
	);
}

function withPrefix(
	session: PanelVisualSelectionSession,
	prefix: string,
): PanelVisualSelectionSession {
	return Object.freeze({ ...session, prefix });
}
