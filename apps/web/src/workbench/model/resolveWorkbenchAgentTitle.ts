import type { TerminalPresentation } from '../components/workbench-panel-workspace/model/panel-layout-types';
import type { WorkbenchAgentNames } from '../types/workbench-agent-observation';

/** What an Agent row may be named from: its terminal's names as this window holds them. */
export type WorkbenchAgentTitleSource = Pick<
	TerminalPresentation,
	'creationLabel' | 'customTitle' | 'naturalTask'
>;

/** What the Server reports for the Agent's current terminal incarnation, if it was read. */
type WorkbenchAgentReportedNames = Pick<
	WorkbenchAgentNames,
	'name' | 'reportedTitle' | 'terminalTitle'
>;

const AUTOMATIC_NUMBER = /^\d+$/u;

/**
 * What names an Agent ahead of its provider, by the TUI Agent row's rule: the name a person gave the
 * Agent; then the title an Agent report or Panel metadata gives the terminal, or else a Panel name a
 * person chose; then what the terminal's own title names; then the label the Server gave the
 * terminal at creation. The terminal's title is the one the Server reports for every terminal, as
 * the TUI reads it, and until that first report the one this window last saw. Blank text, an
 * automatic number and text that only names the provider, as an Agent's idle title does, name
 * nothing; null leaves the provider alone.
 */
export function resolveWorkbenchAgentTitle(
	{ creationLabel, customTitle, naturalTask }: WorkbenchAgentTitleSource,
	reported: WorkbenchAgentReportedNames | undefined,
	providerNames: readonly string[],
): string | null {
	const providerKeys = new Set(providerNames.map(nameKey));
	const candidates: ReadonlyArray<readonly [string | null | undefined, boolean]> = [
		[reported?.name, false],
		[reported?.reportedTitle ?? customTitle, true],
		[reported?.terminalTitle === undefined ? naturalTask : reported.terminalTitle, false],
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
