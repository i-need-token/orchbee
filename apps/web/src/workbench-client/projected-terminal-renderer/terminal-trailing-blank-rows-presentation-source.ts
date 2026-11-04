import type { WorkbenchAgentSource } from '../../workbench';

import type { TerminalTrailingBlankRowsPreferenceSource } from '../../terminal-interaction-settings';

export interface TerminalTrailingBlankRowsPresentationSource {
	current(): boolean;
	subscribe(listener: (enabled: boolean) => void): () => void;
}

export interface CreateTerminalTrailingBlankRowsPresentationSourceOptions {
	readonly agentObservationSource?: WorkbenchAgentSource | undefined;
	readonly preferenceSource?: TerminalTrailingBlankRowsPreferenceSource | undefined;
	readonly presentationId: string;
}

export const DEFAULT_TERMINAL_TRAILING_BLANK_ROWS_PRESENTATION_SOURCE: TerminalTrailingBlankRowsPresentationSource =
	Object.freeze({
		current: () => false,
		subscribe: () => () => undefined,
	});

export function createTerminalTrailingBlankRowsPresentationSource({
	agentObservationSource,
	preferenceSource,
	presentationId,
}: CreateTerminalTrailingBlankRowsPresentationSourceOptions): TerminalTrailingBlankRowsPresentationSource {
	if (agentObservationSource === undefined || preferenceSource === undefined) {
		return DEFAULT_TERMINAL_TRAILING_BLANK_ROWS_PRESENTATION_SOURCE;
	}

	let observable = true;
	const read = (): boolean => {
		if (!observable) return false;
		try {
			// Only a terminal the Server reports an Agent in has its trailing blank rows trimmed.
			return (
				preferenceSource.current() &&
				agentObservationSource.getSnapshot().presentations.has(presentationId)
			);
		} catch {
			return false;
		}
	};

	return {
		current: read,
		subscribe: (listener) => {
			let previous = read();
			let unsubscribeAgent = (): void => undefined;
			let unsubscribePreference = (): void => undefined;
			let subscribed = true;
			const publish = (): void => {
				if (!subscribed) return;
				const next = read();
				if (next === previous) return;
				previous = next;
				try {
					listener(next);
				} catch {
					// A renderer consumer cannot break either application-level source.
				}
			};

			try {
				unsubscribePreference = preferenceSource.subscribe(publish);
				if (typeof unsubscribePreference !== 'function') {
					throw new TypeError('The terminal tail preference source did not unsubscribe.');
				}
				unsubscribeAgent = agentObservationSource.subscribe(publish);
				if (typeof unsubscribeAgent !== 'function') {
					throw new TypeError('The Agent observation source did not unsubscribe.');
				}
			} catch {
				observable = false;
				try {
					unsubscribePreference();
				} catch {
					// Best-effort cleanup after a malformed source.
				}
				try {
					unsubscribeAgent();
				} catch {
					// Best-effort cleanup after a malformed source.
				}
				publish();
				return () => undefined;
			}

			return () => {
				if (!subscribed) return;
				subscribed = false;
				try {
					unsubscribePreference();
				} catch {
					// Application shutdown remains best-effort across external sources.
				}
				try {
					unsubscribeAgent();
				} catch {
					// Application shutdown remains best-effort across external sources.
				}
			};
		},
	};
}
