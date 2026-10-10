import {
	TerminalPresentationIdSchema,
	TerminalPresentationOperationIdSchema,
	TerminalPresentationRemovalIdSchema,
} from '../../terminal-presentation';
import { assertProjectWorkspaceInvariants } from '../../project-workspace';
import { z } from 'zod';

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

const historicalDirectoryBase = {
	available: z.boolean(),
	id: identifierSchema,
	name: z.string(),
	path: z.string().min(1),
	source: z.enum(['created', 'discovered', 'selected']),
};

const historicalDirectorySchema = z.discriminatedUnion('kind', [
	z.strictObject({
		...historicalDirectoryBase,
		git: repositoryGitMetadataSchema.nullable(),
		kind: z.literal('folder'),
	}),
	z.strictObject({
		...historicalDirectoryBase,
		git: worktreeGitMetadataSchema,
		kind: z.literal('worktree'),
	}),
]);

const directoryPlacementSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('local') }),
	z.strictObject({ environmentId: identifierSchema, kind: z.literal('environment') }),
]);

const directoryBase = {
	...historicalDirectoryBase,
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

const historicalWorkspaceSchema = z.strictObject({
	id: identifierSchema,
	launchConfigurationId: nullableIdentifierSchema,
	name: z.string(),
	primaryDirectoryId: identifierSchema,
	terminalContextId: identifierSchema,
});

const historicalProjectVersion2Schema = z.strictObject({
	directories: z.array(directorySchema),
	id: identifierSchema,
	launchConfigurations: z.array(launchConfigurationSchema),
	name: z.string(),
	primaryDirectoryId: identifierSchema,
	workspaces: z.array(historicalWorkspaceSchema),
});

const historicalProjectSchema = z.strictObject({
	directories: z.array(historicalDirectorySchema),
	id: identifierSchema,
	launchConfigurations: z.array(launchConfigurationSchema),
	name: z.string(),
	primaryDirectoryId: identifierSchema,
	workspaces: z.array(historicalWorkspaceSchema),
});

const historicalProjectWorkspaceVersion2Schema = z.strictObject({
	nextProjectSequence: sequenceSchema,
	nextTerminalContextSequence: sequenceSchema,
	nextWorkspaceSequence: sequenceSchema,
	projects: z.array(historicalProjectVersion2Schema),
	standaloneTerminalContextId: identifierSchema,
});

const historicalProjectWorkspaceSchema = z.strictObject({
	nextProjectSequence: sequenceSchema,
	nextTerminalContextSequence: sequenceSchema,
	nextWorkspaceSequence: sequenceSchema,
	projects: z.array(historicalProjectSchema),
	standaloneTerminalContextId: identifierSchema,
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
		.nullable()
		.default(null),
	group: nullableIdentifierSchema,
	id: identifierSchema,
	pinned: z.boolean(),
	title: z.string(),
});

const tabCollectionSchema = z.strictObject({
	groups: z.array(tabGroupSchema),
	tabs: z.array(tabSchema),
});

const presentationIntentSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('provision'),
		operationId: TerminalPresentationOperationIdSchema,
	}),
	z.strictObject({ kind: z.literal('retain') }),
	z.strictObject({
		kind: z.literal('stop'),
		operationId: TerminalPresentationOperationIdSchema,
	}),
	z.strictObject({ kind: z.literal('stopped') }),
	z.strictObject({
		kind: z.literal('close'),
		operationId: TerminalPresentationOperationIdSchema,
		removalId: TerminalPresentationRemovalIdSchema,
	}),
]);

const presentationSchema = z.strictObject({
	intent: presentationIntentSchema,
	launchCwd: z.string().nullable(),
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

const pendingRemovalSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		childPresentationId: TerminalPresentationIdSchema,
		kind: z.literal('child'),
		panelId: identifierSchema,
		presentationIds: z.array(TerminalPresentationIdSchema),
		removalId: TerminalPresentationRemovalIdSchema,
		tabId: identifierSchema,
	}),
	z.strictObject({
		kind: z.literal('panel'),
		panelId: identifierSchema,
		presentationIds: z.array(TerminalPresentationIdSchema),
		removalId: TerminalPresentationRemovalIdSchema,
		tabId: identifierSchema,
	}),
	z.strictObject({
		kind: z.literal('primary'),
		panelId: identifierSchema,
		presentationIds: z.array(TerminalPresentationIdSchema),
		removalId: TerminalPresentationRemovalIdSchema,
		tabId: identifierSchema,
	}),
	z.strictObject({
		kind: z.literal('tab'),
		presentationIds: z.array(TerminalPresentationIdSchema),
		removalId: TerminalPresentationRemovalIdSchema,
		tabId: identifierSchema,
	}),
]);

const panelWorkspaceSchema = z.strictObject({
	nextPanelSequence: sequenceSchema,
	nextPanelSlotSequence: sequenceSchema,
	nextSplitSequence: sequenceSchema,
	nextTerminalSequence: sequenceSchema,
	pendingRemovals: z.array(pendingRemovalSchema),
	tabs: z.array(tabPanelWorkspaceSchema),
});

const terminalContextSchema = z.strictObject({
	nextTabSequence: sequenceSchema,
	panelWorkspace: panelWorkspaceSchema,
	tabCollection: tabCollectionSchema,
	terminalContextId: identifierSchema,
});

export type WorkbenchSharedTerminalContext = z.infer<typeof terminalContextSchema>;

export function createEmptyWorkbenchSharedTerminalContext(
	terminalContextId: string,
): WorkbenchSharedTerminalContext {
	return terminalContextSchema.parse({
		nextTabSequence: 1,
		panelWorkspace: {
			nextPanelSequence: 1,
			nextPanelSlotSequence: 1,
			nextSplitSequence: 1,
			nextTerminalSequence: 1,
			pendingRemovals: [],
			tabs: [],
		},
		tabCollection: { groups: [], tabs: [] },
		terminalContextId,
	});
}

export const WorkbenchSharedDocumentRevisionSchema = z
	.string()
	.regex(/^(?:0|[1-9][0-9]*)$/u)
	.brand<'WorkbenchSharedDocumentRevision'>();
export type WorkbenchSharedDocumentRevision = z.infer<typeof WorkbenchSharedDocumentRevisionSchema>;

const workbenchSharedDocumentBaseSchema = z.strictObject({
	projectWorkspace: projectWorkspaceSchema,
	revision: WorkbenchSharedDocumentRevisionSchema,
	terminalContexts: z.array(terminalContextSchema),
	version: z.literal(3),
});

const historicalWorkbenchSharedDocumentVersion1Schema = z.strictObject({
	projectWorkspace: historicalProjectWorkspaceSchema,
	revision: WorkbenchSharedDocumentRevisionSchema,
	terminalContexts: z.array(terminalContextSchema),
	version: z.literal(1),
});

const historicalWorkbenchSharedDocumentVersion2BaseSchema = z.strictObject({
	projectWorkspace: historicalProjectWorkspaceVersion2Schema,
	revision: WorkbenchSharedDocumentRevisionSchema,
	terminalContexts: z.array(terminalContextSchema),
	version: z.literal(2),
});
const historicalWorkbenchSharedDocumentVersion2Schema =
	historicalWorkbenchSharedDocumentVersion2BaseSchema.superRefine((document, context) => {
		for (const message of validateHistoricalDocument(document))
			context.addIssue({ code: 'custom', message });
	});

export const WorkbenchSharedDocumentSchema = workbenchSharedDocumentBaseSchema.superRefine(
	(document, context) => {
		for (const issue of validateDocument(document)) {
			context.addIssue({ code: 'custom', message: issue });
		}
	},
);

export type WorkbenchSharedDocument = z.infer<typeof WorkbenchSharedDocumentSchema>;
export type WorkbenchSharedDocumentInput = Omit<WorkbenchSharedDocument, 'version'>;

export function createWorkbenchSharedDocument(
	input: WorkbenchSharedDocumentInput,
): WorkbenchSharedDocument {
	return WorkbenchSharedDocumentSchema.parse({
		...input,
		terminalContexts: [...input.terminalContexts].sort((left, right) =>
			compareIdentifiers(left.terminalContextId, right.terminalContextId),
		),
		version: 3,
	});
}

export function parseWorkbenchSharedDocument(value: unknown): WorkbenchSharedDocument {
	const version = z.object({ version: z.number() }).parse(value).version;
	if (version === 1) return migrateWorkbenchSharedDocumentVersion1(value);
	if (version === 2) return migrateWorkbenchSharedDocumentVersion2(value);
	return WorkbenchSharedDocumentSchema.parse(value);
}

export function migrateWorkbenchSharedDocumentVersion1(value: unknown): WorkbenchSharedDocument {
	const historical = historicalWorkbenchSharedDocumentVersion1Schema.parse(value);
	return migrateWorkbenchSharedDocumentVersion2({
		...historical,
		projectWorkspace: {
			...historical.projectWorkspace,
			projects: historical.projectWorkspace.projects.map((project) => ({
				...project,
				directories: project.directories.map((directory) => ({
					...directory,
					placement: { kind: 'local' },
				})),
			})),
		},
		version: 2,
	});
}

export function migrateWorkbenchSharedDocumentVersion2(value: unknown): WorkbenchSharedDocument {
	// Validate the complete historical graph before giving it new identities or relationships.
	const historical = historicalWorkbenchSharedDocumentVersion2Schema.parse(value);
	const navigation = historical.projectWorkspace;
	return WorkbenchSharedDocumentSchema.parse({
		...historical,
		projectWorkspace: {
			nextProjectSequence: navigation.nextProjectSequence,
			nextResourceScopeSequence: 1,
			nextTerminalContextSequence: navigation.nextTerminalContextSequence,
			nextWorkspaceSequence: navigation.nextWorkspaceSequence,
			projects: navigation.projects.map(({ id, name }) => ({ id, name })),
			resourceScopes: navigation.projects.map(({ workspaces: _workspaces, ...scope }) => scope),
			standaloneTerminalContextId: navigation.standaloneTerminalContextId,
			workspaces: navigation.projects.flatMap((project) =>
				project.workspaces.map((workspace) => ({
					...workspace,
					projectId: project.id,
					resourceScopeId: project.id,
				})),
			),
		},
		version: 3,
	});
}

export function workbenchSharedDocumentRevision(value: string): WorkbenchSharedDocumentRevision {
	return WorkbenchSharedDocumentRevisionSchema.parse(value);
}

function validateHistoricalDocument(
	document: z.infer<typeof historicalWorkbenchSharedDocumentVersion2BaseSchema>,
): readonly string[] {
	const issues: string[] = [];
	const projectIds = new Set<string>();
	const workspaceIds = new Set<string>();
	const contextIds = new Set<string>([document.projectWorkspace.standaloneTerminalContextId]);
	const remoteLocations = new Set<string>();
	for (const project of document.projectWorkspace.projects) {
		if (projectIds.has(project.id)) issues.push(`Duplicate historical Project ${project.id}.`);
		projectIds.add(project.id);
		const directoryIds = new Set<string>();
		const locations = new Set<string>();
		const environments = new Set<string>();
		for (const directory of project.directories) {
			if (directoryIds.has(directory.id))
				issues.push(`Duplicate historical Directory ${directory.id}.`);
			directoryIds.add(directory.id);
			const location = JSON.stringify([directory.placement, directory.path]);
			if (locations.has(location))
				issues.push(`Duplicate historical Directory path ${directory.path}.`);
			locations.add(location);
			if (directory.placement.kind === 'environment') {
				environments.add(directory.placement.environmentId);
				if (directory.available)
					issues.push('A historical remote Directory cannot be locally available.');
				if (remoteLocations.has(location))
					issues.push('Duplicate historical remote Directory placement.');
				remoteLocations.add(location);
			}
		}
		if (environments.size > 1) issues.push(`Historical Project ${project.id} spans Environments.`);
		if (!directoryIds.has(project.primaryDirectoryId))
			issues.push(`Historical Project ${project.id} has a missing primary Directory.`);
		const configurationIds = new Set<string>();
		for (const configuration of project.launchConfigurations) {
			if (configurationIds.has(configuration.id) || configuration.name.trim().length === 0)
				issues.push('Invalid historical launch configuration.');
			configurationIds.add(configuration.id);
		}
		for (const workspace of project.workspaces) {
			if (workspaceIds.has(workspace.id))
				issues.push(`Duplicate historical Workspace ${workspace.id}.`);
			workspaceIds.add(workspace.id);
			if (!directoryIds.has(workspace.primaryDirectoryId))
				issues.push(`Historical Workspace ${workspace.id} has a missing Directory.`);
			if (
				workspace.launchConfigurationId !== null &&
				!configurationIds.has(workspace.launchConfigurationId)
			)
				issues.push(`Historical Workspace ${workspace.id} has a missing launch configuration.`);
			if (contextIds.has(workspace.terminalContextId))
				issues.push(`Duplicate historical terminal context ${workspace.terminalContextId}.`);
			contextIds.add(workspace.terminalContextId);
		}
	}
	return [...issues, ...validateDocumentContexts(document.terminalContexts, contextIds)];
}

function validateDocument(
	document: z.infer<typeof workbenchSharedDocumentBaseSchema>,
): readonly string[] {
	const issues: string[] = [];
	try {
		assertProjectWorkspaceInvariants({
			...document.projectWorkspace,
			activeLocation: { kind: 'standalone' },
			expandedProjectIds: [],
		});
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
	const presentations = new Map<string, WorkbenchSharedPresentation>();
	const panelsByTab = new Map<string, Map<string, WorkbenchSharedPanelLeaf>>();
	for (const tab of terminalContext.panelWorkspace.tabs) {
		const localPanels = new Map<string, WorkbenchSharedPanelLeaf>();
		visitPanelNode(
			tab.root,
			panelIds,
			splitIds,
			slotIds,
			presentations,
			globalPresentationIds,
			localPanels,
			issues,
		);
		panelsByTab.set(tab.tabId, localPanels);
	}
	const removalIds = new Set<string>();
	const removalPresentationIds = new Set<string>();
	for (const removal of terminalContext.panelWorkspace.pendingRemovals) {
		if (removalIds.has(removal.removalId)) {
			issues.push(`Duplicate pending removal: ${removal.removalId}.`);
		}
		removalIds.add(removal.removalId);
		if (removal.presentationIds.length === 0) {
			issues.push(`Pending removal ${removal.removalId} has no presentation.`);
		}
		const tab = terminalContext.panelWorkspace.tabs.find(
			(candidate) => candidate.tabId === removal.tabId,
		);
		if (tab === undefined) {
			issues.push(`Pending removal ${removal.removalId} references a missing Tab.`);
			continue;
		}
		const expectedIds = expectedRemovalPresentationIds(
			removal,
			tab.root,
			panelsByTab.get(removal.tabId),
		);
		if (expectedIds === null || !setsEqual(new Set(removal.presentationIds), expectedIds)) {
			issues.push(`Pending removal ${removal.removalId} has an invalid target set.`);
		}
		for (const presentationId of removal.presentationIds) {
			if (removalPresentationIds.has(presentationId)) {
				issues.push(`Presentation ${presentationId} belongs to overlapping removals.`);
			}
			removalPresentationIds.add(presentationId);
			const presentation = presentations.get(presentationId);
			if (
				presentation?.intent.kind !== 'close' ||
				presentation.intent.removalId !== removal.removalId
			) {
				issues.push(`Presentation ${presentationId} has no matching close intent.`);
			}
		}
	}
	for (const presentation of presentations.values()) {
		if (
			presentation.intent.kind === 'close' &&
			!removalPresentationIds.has(presentation.presentationId)
		) {
			issues.push(`Close intent ${presentation.presentationId} has no pending removal.`);
		}
	}
}

function visitPanelNode(
	node: WorkbenchSharedPanelNode,
	panelIds: Set<string>,
	splitIds: Set<string>,
	slotIds: Set<string>,
	presentations: Map<string, WorkbenchSharedPresentation>,
	globalPresentationIds: Set<string>,
	localPanels: Map<string, WorkbenchSharedPanelLeaf>,
	issues: string[],
): void {
	if (node.kind === 'panel') {
		if (panelIds.has(node.id)) issues.push(`Duplicate Panel: ${node.id}.`);
		panelIds.add(node.id);
		localPanels.set(node.id, node);
		for (const presentation of [node.primaryPresentation, ...node.childPresentations]) {
			if (globalPresentationIds.has(presentation.presentationId)) {
				issues.push(`Duplicate TerminalPresentationId: ${presentation.presentationId}.`);
			}
			globalPresentationIds.add(presentation.presentationId);
			presentations.set(presentation.presentationId, presentation);
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
		visitPanelNode(
			child.node,
			panelIds,
			splitIds,
			slotIds,
			presentations,
			globalPresentationIds,
			localPanels,
			issues,
		);
	}
}

function expectedRemovalPresentationIds(
	removal: z.infer<typeof pendingRemovalSchema>,
	root: WorkbenchSharedPanelNode,
	panels: ReadonlyMap<string, WorkbenchSharedPanelLeaf> | undefined,
): Set<string> | null {
	if (removal.kind === 'tab') return new Set(collectPresentations(root));
	const panel = panels?.get(removal.panelId);
	if (panel === undefined) return null;
	if (removal.kind === 'child') {
		return panel.childPresentations.some(
			(presentation) => presentation.presentationId === removal.childPresentationId,
		)
			? new Set([removal.childPresentationId])
			: null;
	}
	if (removal.kind === 'primary') {
		return new Set([panel.primaryPresentation.presentationId]);
	}
	return new Set([
		panel.primaryPresentation.presentationId,
		...panel.childPresentations.map((presentation) => presentation.presentationId),
	]);
}

function collectPresentations(node: WorkbenchSharedPanelNode): readonly string[] {
	if (node.kind === 'panel') {
		return [
			node.primaryPresentation.presentationId,
			...node.childPresentations.map((presentation) => presentation.presentationId),
		];
	}
	return node.children.flatMap((child) => collectPresentations(child.node));
}

function arraysEqual(left: readonly string[], right: readonly string[]): boolean {
	return left.length === right.length && left.every((value, index) => value === right[index]);
}

function setsEqual(left: ReadonlySet<string>, right: ReadonlySet<string>): boolean {
	return left.size === right.size && [...left].every((value) => right.has(value));
}

function compareIdentifiers(left: string, right: string): number {
	if (left < right) return -1;
	return left === right ? 0 : 1;
}
