import type { TerminalPresentation } from '../components/workbench-panel-workspace/model/panel-layout-types';

/** What an Agent row may be named from: its terminal's names as this window holds them. */
export type WorkbenchAgentTitleSource = Pick<
	TerminalPresentation,
	'creationLabel' | 'customTitle' | 'naturalTask'
>;

const AUTOMATIC_NUMBER = /^\d+$/u;

/**
 * What names an Agent ahead of its provider, by the TUI Agent row's rule: a Panel name a person
 * chose, then what the terminal's own title names, then the label the Server gave the terminal at
 * creation. Blank text, an automatic number and text that only names the provider, as an Agent's
 * idle title does, name nothing; null leaves the provider alone. The TUI first takes a name a
 * person gave the Agent and an Agent report's title, which a Client does not read from the Server.
 */
export function resolveWorkbenchAgentTitle(
	{ creationLabel, customTitle, naturalTask }: WorkbenchAgentTitleSource,
	providerNames: readonly string[],
): string | null {
	const providerKeys = new Set(providerNames.map(nameKey));
	const candidates: ReadonlyArray<readonly [string | null | undefined, boolean]> = [
		[customTitle, true],
		[naturalTask, false],
		[customTitle === undefined ? creationLabel : undefined, true],
	];
	for (const [candidate, numbered] of candidates) {
		const text = candidate?.trim() ?? '';
		if (text === '' || (numbered && AUTOMATIC_NUMBER.test(text))) continue;
		if (!providerKeys.has(nameKey(text))) return text;
	}
	return null;
}

/** Compares names ignoring case, spacing and punctuation, so `Claude Code` names `claude-code`. */
function nameKey(name: string): string {
	return name.replaceAll(/[^\p{Alphabetic}\p{N}]/gu, '').toLowerCase();
}
