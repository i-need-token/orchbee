import { z } from 'zod';

import { TerminalPresentationIdSchema } from './terminal-presentation-contract';
import { assertWorkbenchSharedProjectWorkspaceInvariants } from './assertWorkbenchSharedProjectWorkspaceInvariants';

const identifierSchema = z.string().min(1);
const sequenceSchema = z.number().int().positive();
const nullableIdentifierSchema = identifierSchema.nullable();

const repositoryGitMetadataSchema = z.strictObject({
	branch: z.string().nullable(),
	head: z.string(),
	repositoryId: identifierSchema,
	worktree: z.literal(false),
});

const worktreeGitMetadataSchema = z.strictObject({
	branch: z.string().nullable(),
	head: z.string(),
	repositoryId: identifierSchema,
	worktree: z.literal(true),
});

const directoryPlacementSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('local') }),
	z.strictObject({ environmentId: identifierSchema, kind: z.literal('environment') }),
]);

const directoryBase = {
	available: z.boolean(),
	id: identifierSchema,
	name: z.string(),
	path: z.string().min(1),
	source: z.enum(['created', 'discovered', 'selected']),
	placement: directoryPlacementSchema,
};

const directorySchema = z.discriminatedUnion('kind', [
	z.strictObject({
		...directoryBase,
		git: repositoryGitMetadataSchema.nullable(),
		kind: z.literal('folder'),
	}),
	z.strictObject({
		...directoryBase,
		git: worktreeGitMetadataSchema,
		kind: z.literal('worktree'),
	}),
]);

const launchConfigurationSchema = z.strictObject({
	id: identifierSchema,
	name: z.string(),
});

const workspaceSchema = z.strictObject({
	id: identifierSchema,
	launchConfigurationId: nullableIdentifierSchema,
	name: z.string(),
	primaryDirectoryId: identifierSchema,
	projectId: nullableIdentifierSchema,
	resourceScopeId: identifierSchema,
	terminalContextId: identifierSchema,
});
const resourceScopeSchema = z.strictObject({
	directories: z.array(directorySchema),
	id: identifierSchema,
	launchConfigurations: z.array(launchConfigurationSchema),
	name: z.string(),
	primaryDirectoryId: identifierSchema,
});
const projectWorkspaceSchema = z.strictObject({
	nextProjectSequence: sequenceSchema,
	nextResourceScopeSequence: sequenceSchema,
	nextTerminalContextSequence: sequenceSchema,
	nextWorkspaceSequence: sequenceSchema,
	projects: z.array(z.strictObject({ id: identifierSchema, name: z.string() })),
	resourceScopes: z.array(resourceScopeSchema),
	standaloneTerminalContextId: identifierSchema,
	workspaces: z.array(workspaceSchema),
});

const tabGroupColorSchema = z.enum([
	'grey',
	'blue',
	'red',
	'yellow',
	'green',
	'pink',
	'purple',
	'cyan',
	'orange',
]);

const tabGroupSchema = z.strictObject({
	color: tabGroupColorSchema,
	id: identifierSchema,
	title: z.string(),
});

const tabSchema = z.strictObject({
	customTitle: z
		.string()
		.min(1)
		.refine((value) => value === value.trim(), 'A custom Tab title must be trimmed.')
		.nullable(),
	group: nullableIdentifierSchema,
	id: identifierSchema,
	pinned: z.boolean(),
	title: z.string(),
});

const tabCollectionSchema = z.strictObject({
	groups: z.array(tabGroupSchema),
	tabs: z.array(tabSchema),
});

/** `retain` keeps the terminal bound to a Presentation; `provision` marks one that has none. */
const presentationIntentSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('provision') }),
	z.strictObject({ kind: z.literal('retain') }),
]);

const presentationSchema = z.strictObject({
	intent: presentationIntentSchema,
	presentationId: TerminalPresentationIdSchema,
	title: z.string(),
});

export type WorkbenchSharedPresentationIntent = z.infer<typeof presentationIntentSchema>;
export type WorkbenchSharedPresentation = z.infer<typeof presentationSchema>;

interface WorkbenchSharedPanelLeaf {
	readonly childPresentations: WorkbenchSharedPresentation[];
	readonly id: string;
	readonly kind: 'panel';
	readonly primaryPresentation: WorkbenchSharedPresentation;
}

interface WorkbenchSharedPanelSplitChild {
	readonly id: string;
	readonly node: WorkbenchSharedPanelNode;
}

interface WorkbenchSharedPanelSplit {
	readonly children: WorkbenchSharedPanelSplitChild[];
	readonly id: string;
	readonly kind: 'split';
	readonly orientation: 'horizontal' | 'vertical';
}

export type WorkbenchSharedPanelNode = WorkbenchSharedPanelLeaf | WorkbenchSharedPanelSplit;

const panelNodeSchema: z.ZodType<WorkbenchSharedPanelNode> = z.lazy(() =>
	z.discriminatedUnion('kind', [
		z.strictObject({
			childPresentations: z.array(presentationSchema),
			id: identifierSchema,
			kind: z.literal('panel'),
			primaryPresentation: presentationSchema,
		}),
		z.strictObject({
			children: z
				.array(
					z.strictObject({
						id: identifierSchema,
						node: panelNodeSchema,
					}),
				)
				.length(2),
			id: identifierSchema,
			kind: z.literal('split'),
			orientation: z.enum(['horizontal', 'vertical']),
		}),
	]),
);

const tabPanelWorkspaceSchema = z.strictObject({
	root: panelNodeSchema,
	tabId: identifierSchema,
});

const panelWorkspaceSchema = z.strictObject({
	nextPanelSequence: sequenceSchema,
	nextPanelSlotSequence: sequenceSchema,
	nextSplitSequence: sequenceSchema,
	nextTerminalSequence: sequenceSchema,
	tabs: z.array(tabPanelWorkspaceSchema),
});

const terminalContextSchema = z.strictObject({
	nextTabSequence: sequenceSchema,
	panelWorkspace: panelWorkspaceSchema,
	tabCollection: tabCollectionSchema,
	terminalContextId: identifierSchema,
});

type WorkbenchSharedTerminalContext = z.infer<typeof terminalContextSchema>;

const WorkbenchSharedDocumentRevisionSchema = z
	.string()
	.regex(/^(?:0|[1-9][0-9]*)$/u)
	.brand<'WorkbenchSharedDocumentRevision'>();
export type WorkbenchSharedDocumentRevision = z.infer<typeof WorkbenchSharedDocumentRevisionSchema>;

const workbenchSharedDocumentBaseSchema = z.strictObject({
	projectWorkspace: projectWorkspaceSchema,
	revision: WorkbenchSharedDocumentRevisionSchema,
	terminalContexts: z.array(terminalContextSchema),
	version: z.literal(1),
});

const WorkbenchSharedDocumentSchema = workbenchSharedDocumentBaseSchema.superRefine(
	(document, context) => {
		for (const issue of validateDocument(document)) {
			context.addIssue({ code: 'custom', message: issue });
		}
	},
);

export type WorkbenchSharedDocument = z.infer<typeof WorkbenchSharedDocumentSchema>;
type WorkbenchSharedDocumentInput = Omit<WorkbenchSharedDocument, 'version'>;

export function createWorkbenchSharedDocument(
	input: WorkbenchSharedDocumentInput,
): WorkbenchSharedDocument {
	return WorkbenchSharedDocumentSchema.parse({
		...input,
		terminalContexts: [...input.terminalContexts].sort((left, right) =>
			compareIdentifiers(left.terminalContextId, right.terminalContextId),
		),
		version: 1,
	});
}

export function parseWorkbenchSharedDocument(value: unknown): WorkbenchSharedDocument {
	return WorkbenchSharedDocumentSchema.parse(value);
}

export function workbenchSharedDocumentRevision(value: string): WorkbenchSharedDocumentRevision {
	return WorkbenchSharedDocumentRevisionSchema.parse(value);
}

function validateDocument(
	document: z.infer<typeof workbenchSharedDocumentBaseSchema>,
): readonly string[] {
	const issues: string[] = [];
	try {
		assertWorkbenchSharedProjectWorkspaceInvariants(document.projectWorkspace);
	} catch (error) {
		issues.push(
			error instanceof Error ? error.message : 'Invalid shared Project and Workspace structure.',
		);
	}
	const referencedContextIds = new Set<string>([
		document.projectWorkspace.standaloneTerminalContextId,
		...document.projectWorkspace.workspaces.map((workspace) => workspace.terminalContextId),
	]);
	return [...issues, ...validateDocumentContexts(document.terminalContexts, referencedContextIds)];
}

function validateDocumentContexts(
	terminalContexts: readonly WorkbenchSharedTerminalContext[],
	referencedContextIds: ReadonlySet<string>,
): readonly string[] {
	const issues: string[] = [];
	const contextIds = new Set<string>();
	const presentationIds = new Set<string>();
	let previousContextId: string | undefined;
	for (const terminalContext of terminalContexts) {
		if (
			previousContextId !== undefined &&
			compareIdentifiers(previousContextId, terminalContext.terminalContextId) >= 0
		) {
			issues.push('Terminal contexts must use unique canonical identity order.');
		}
		previousContextId = terminalContext.terminalContextId;
		if (contextIds.has(terminalContext.terminalContextId)) {
			issues.push(`Duplicate terminal context: ${terminalContext.terminalContextId}.`);
		}
		contextIds.add(terminalContext.terminalContextId);
		validateTerminalContext(terminalContext, presentationIds, issues);
	}
	for (const referencedContextId of referencedContextIds) {
		if (!contextIds.has(referencedContextId)) {
			issues.push(`Missing terminal context: ${referencedContextId}.`);
		}
	}
	for (const contextId of contextIds) {
		if (!referencedContextIds.has(contextId)) {
			issues.push(`Unreferenced terminal context: ${contextId}.`);
		}
	}
	return issues;
}

function validateTerminalContext(
	terminalContext: WorkbenchSharedTerminalContext,
	globalPresentationIds: Set<string>,
	issues: string[],
): void {
	const tabIds = new Set<string>();
	const groupIds = new Set<string>();
	for (const group of terminalContext.tabCollection.groups) {
		if (groupIds.has(group.id)) issues.push(`Duplicate Tab group: ${group.id}.`);
		groupIds.add(group.id);
	}
	let foundUnpinnedTab = false;
	const groupBounds = new Map<string, { first: number; last: number; members: number }>();
	for (const [index, tab] of terminalContext.tabCollection.tabs.entries()) {
		if (tabIds.has(tab.id)) issues.push(`Duplicate Tab: ${tab.id}.`);
		tabIds.add(tab.id);
		if (foundUnpinnedTab && tab.pinned) issues.push('Pinned Tabs must form one leading prefix.');
		if (!tab.pinned) foundUnpinnedTab = true;
		if (tab.pinned && tab.group !== null) issues.push(`Pinned Tab ${tab.id} cannot be grouped.`);
		if (tab.group !== null) {
			if (!groupIds.has(tab.group)) issues.push(`Tab ${tab.id} references a missing group.`);
			const bounds = groupBounds.get(tab.group);
			groupBounds.set(
				tab.group,
				bounds === undefined
					? { first: index, last: index, members: 1 }
					: { first: bounds.first, last: index, members: bounds.members + 1 },
			);
		}
	}
	for (const [groupId, bounds] of groupBounds) {
		if (bounds.last - bounds.first + 1 !== bounds.members) {
			issues.push(`Tab group ${groupId} must be contiguous.`);
		}
	}
	const panelTabIds = terminalContext.panelWorkspace.tabs.map((tab) => tab.tabId);
	const orderedTabIds = terminalContext.tabCollection.tabs.map((tab) => tab.id);
	if (!arraysEqual(panelTabIds, orderedTabIds)) {
		issues.push('Tab and Panel workspace membership and order must match.');
	}
	validatePanelWorkspace(terminalContext, globalPresentationIds, issues);
}

function validatePanelWorkspace(
	terminalContext: WorkbenchSharedTerminalContext,
	globalPresentationIds: Set<string>,
	issues: string[],
): void {
	const panelIds = new Set<string>();
	const splitIds = new Set<string>();
	const slotIds = new Set<string>();
	for (const tab of terminalContext.panelWorkspace.tabs) {
		visitPanelNode(tab.root, panelIds, splitIds, slotIds, globalPresentationIds, issues);
	}
}

function visitPanelNode(
	node: WorkbenchSharedPanelNode,
	panelIds: Set<string>,
	splitIds: Set<string>,
	slotIds: Set<string>,
	globalPresentationIds: Set<string>,
	issues: string[],
): void {
	if (node.kind === 'panel') {
		if (panelIds.has(node.id)) issues.push(`Duplicate Panel: ${node.id}.`);
		panelIds.add(node.id);
		for (const presentation of [node.primaryPresentation, ...node.childPresentations]) {
			if (globalPresentationIds.has(presentation.presentationId)) {
				issues.push(`Duplicate TerminalPresentationId: ${presentation.presentationId}.`);
			}
			globalPresentationIds.add(presentation.presentationId);
		}
		return;
	}
	if (splitIds.has(node.id)) issues.push(`Duplicate Split: ${node.id}.`);
	splitIds.add(node.id);
	if (node.children.length !== 2) {
		issues.push(`Split ${node.id} does not have exactly two children.`);
	}
	for (const child of node.children) {
		if (slotIds.has(child.id)) issues.push(`Duplicate Panel slot: ${child.id}.`);
		slotIds.add(child.id);
		visitPanelNode(child.node, panelIds, splitIds, slotIds, globalPresentationIds, issues);
	}
}

function arraysEqual(left: readonly string[], right: readonly string[]): boolean {
	return left.length === right.length && left.every((value, index) => value === right[index]);
}

function compareIdentifiers(left: string, right: string): number {
	if (left < right) return -1;
	return left === right ? 0 : 1;
}
