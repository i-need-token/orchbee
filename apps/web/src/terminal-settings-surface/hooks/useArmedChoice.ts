import * as React from 'react';

/** How long a question that opened on its own shows, with no key pressed on it, before a choice counts. */
const ARMING_MILLISECONDS = 1000;

/**
 * Whether a question that opened on its own has shown for a second since `phase` began, the window
 * last came into view, or `restart` last ran, so an Enter meant for a terminal never answers it.
 */
export function useArmedChoice(phase: string): {
	readonly now: boolean;
	readonly restart: () => void;
} {
	const [round, setRound] = React.useState(0);
	const [armedAt, setArmedAt] = React.useState<string | null>(null);
	const key = `${phase}:${round}`;
	React.useEffect(() => {
		const timer = window.setTimeout(() => setArmedAt(key), ARMING_MILLISECONDS);
		return () => window.clearTimeout(timer);
	}, [key]);
	React.useEffect(() => {
		const shown = (): void => {
			if (document.visibilityState === 'visible') setRound((value) => value + 1);
		};
		document.addEventListener('visibilitychange', shown);
		return () => document.removeEventListener('visibilitychange', shown);
	}, []);
	return { now: armedAt === key, restart: () => setRound((value) => value + 1) };
}
