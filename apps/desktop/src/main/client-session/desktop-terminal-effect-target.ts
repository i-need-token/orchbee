import type {
	WorkbenchSharedDocument,
	WorkbenchSharedPanelNode,
} from '@orchbee/workbench/document';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';

export type DesktopTerminalEffectTarget = { readonly kind: 'local' };

export function resolveDesktopTerminalContextEffectTarget(
	document: WorkbenchSharedDocument,
	terminalContextId: string,
): DesktopTerminalEffectTarget {
	const matches: DesktopTerminalEffectTarget[] = [];
	if (document.projectWorkspace.standaloneTerminalContextId === terminalContextId) {
		matches.push({ kind: 'local' });
	}
	for (const project of document.projectWorkspace.projects) {
		for (const workspace of project.workspaces) {
			if (workspace.terminalContextId !== terminalContextId) continue;
			const directory = project.directories.find(
				(candidate) => candidate.id === workspace.primaryDirectoryId,
			);
			if (directory === undefined) {
				throw new Error('A Workspace terminal target requires its primary Directory.');
			}
			if (directory.placement.kind !== 'local') {
				throw new Error('Desktop terminal effects require a local Directory.');
			}
			matches.push({ kind: 'local' });
		}
	}
	if (matches.length !== 1) {
		throw new Error('A Terminal Context must resolve to exactly one effect target.');
	}
	const match = matches[0];
	if (match === undefined) throw new Error('Terminal target resolution lost its match.');
	return match;
}

export function resolveDesktopTerminalPresentationEffectTarget(
	document: WorkbenchSharedDocument,
	presentationId: TerminalPresentationId,
): DesktopTerminalEffectTarget {
	let matchCount = 0;
	let terminalContextId: string | undefined;
	for (const context of document.terminalContexts) {
		const contextMatches = context.panelWorkspace.tabs.reduce(
			(total, tab) => total + countPresentationOccurrences(tab.root, presentationId),
			0,
		);
		if (contextMatches === 0) continue;
		matchCount += contextMatches;
		terminalContextId = context.terminalContextId;
	}
	if (matchCount !== 1) {
		throw new Error('A terminal Presentation must resolve to exactly one Terminal Context.');
	}
	if (terminalContextId === undefined) {
		throw new Error('Terminal Presentation target resolution lost its context.');
	}
	return resolveDesktopTerminalContextEffectTarget(document, terminalContextId);
}

function countPresentationOccurrences(
	node: WorkbenchSharedPanelNode,
	presentationId: TerminalPresentationId,
): number {
	if (node.kind === 'panel') {
		return (
			Number(node.primaryPresentation.presentationId === presentationId) +
			node.childPresentations.filter(
				(presentation) => presentation.presentationId === presentationId,
			).length
		);
	}
	return node.children.reduce(
		(total, child) => total + countPresentationOccurrences(child.node, presentationId),
		0,
	);
}
