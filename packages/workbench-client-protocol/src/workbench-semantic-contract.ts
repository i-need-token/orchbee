import { z } from 'zod';

export const WORKBENCH_PROTOCOL_VERSION = 1;
export const WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES = 16 * 1024;
export const WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES = 1024 * 1024;
export const WORKBENCH_MAXIMUM_LABEL_LENGTH = 512;
export const WORKBENCH_MAXIMUM_PROJECTS = 256;
export const WORKBENCH_MAXIMUM_DIRECTORIES_PER_PROJECT = 256;
export const WORKBENCH_MAXIMUM_DIRECTORY_PATH_BYTES = 4096;
export const WORKBENCH_MAXIMUM_WORKSPACES_PER_PROJECT = 256;
export const WORKBENCH_MAXIMUM_TERMINAL_CONTEXTS = 1024;
export const WORKBENCH_MAXIMUM_TAB_GROUPS_PER_CONTEXT = 64;
export const WORKBENCH_MAXIMUM_TABS_PER_CONTEXT = 256;
export const WORKBENCH_MAXIMUM_CHILD_PRESENTATIONS_PER_PANEL = 16;
export const WORKBENCH_MAXIMUM_PANEL_NODE_DEPTH = 16;
export const WORKBENCH_MAXIMUM_PANEL_NODES_PER_CONTEXT = 4096;
export const WORKBENCH_MAXIMUM_DELTA_CHANGES = 1024;

const MAXIMUM_UINT64_TEXT = '18446744073709551615';
const ZERO_IDENTITY = '00000000000000000000000000000000';
const MAXIMUM_DECODED_VALUE_DEPTH = 64;
const excessiveDepthSentinel = Symbol('orchbee-workbench-excessive-value-depth');

const identitySchema = z
	.string()
	.regex(/^[0-9a-f]{32}$/u)
	.refine((value) => value !== ZERO_IDENTITY, 'Identity must not use the zero sentinel');

export const SiteIdSchema = identitySchema.brand<'SiteId'>();
export const WorkbenchRequestIdSchema = identitySchema.brand<'WorkbenchRequestId'>();
export const WorkbenchProjectIdSchema = identitySchema.brand<'WorkbenchProjectId'>();
export const WorkbenchWorkspaceIdSchema = identitySchema.brand<'WorkbenchWorkspaceId'>();
export const WorkbenchDirectoryIdSchema = identitySchema.brand<'WorkbenchDirectoryId'>();
export const WorkbenchTerminalContextIdSchema =
	identitySchema.brand<'WorkbenchTerminalContextId'>();
export const WorkbenchTabGroupIdSchema = identitySchema.brand<'WorkbenchTabGroupId'>();
export const WorkbenchTabIdSchema = identitySchema.brand<'WorkbenchTabId'>();
export const WorkbenchPanelIdSchema = identitySchema.brand<'WorkbenchPanelId'>();
export const WorkbenchSplitIdSchema = identitySchema.brand<'WorkbenchSplitId'>();
export const WorkbenchPanelSlotIdSchema = identitySchema.brand<'WorkbenchPanelSlotId'>();
export const TerminalPresentationIdSchema = identitySchema.brand<'TerminalPresentationId'>();
export const WorkerIdSchema = identitySchema.brand<'WorkerId'>();
export const TerminalIdSchema = identitySchema.brand<'TerminalId'>();

export const WorkbenchRevisionSchema = z
	.string()
	.regex(/^(?:0|[1-9][0-9]{0,19})$/u)
	.refine(
		(value) =>
			value.length < MAXIMUM_UINT64_TEXT.length ||
			(value.length === MAXIMUM_UINT64_TEXT.length && value <= MAXIMUM_UINT64_TEXT),
		'Revision exceeds unsigned 64-bit range',
	)
	.brand<'WorkbenchRevision'>();

export const WorkbenchTabGroupLabelSchema = z
	.string()
	.refine(
		(value) => !/[\uD800-\uDFFF]/u.test(value),
		'Label contains an unpaired Unicode surrogate',
	)
	.refine(
		(value) => [...value].length <= WORKBENCH_MAXIMUM_LABEL_LENGTH,
		'Label exceeds the Unicode code-point limit',
	);

export const WorkbenchLabelSchema = WorkbenchTabGroupLabelSchema.refine(
	(value) => value.length > 0,
	'Label must not be empty',
);

const WorkbenchPanelCustomLabelSchema = WorkbenchLabelSchema.refine(
	(value) => !/^\p{White_Space}|\p{White_Space}$/u.test(value),
	'Panel custom label must already be trimmed',
);

export const WorkbenchPanelLabelUpdateSchema = z
	.string()
	.refine(
		(value) => !/[\uD800-\uDFFF]/u.test(value),
		'Label contains an unpaired Unicode surrogate',
	)
	.refine(
		(value) => [...value].length <= WORKBENCH_MAXIMUM_LABEL_LENGTH,
		'Label exceeds the Unicode code-point limit',
	)
	.nullish()
	.transform((value) => value?.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '') || null);

export const WorkbenchDirectoryPathSchema = z
	.string()
	.min(1)
	.max(WORKBENCH_MAXIMUM_DIRECTORY_PATH_BYTES)
	.refine(
		(value) => /^(?:\/|(?:\\\\\?\\)?[A-Za-z]:[/\\])/u.test(value),
		'Directory path must be absolute local-platform text',
	)
	.refine(
		(value) => !value.includes('\0') && !/[\uD800-\uDFFF]/u.test(value),
		'Directory path must contain UTF-8 text without NUL',
	)
	.refine(
		(value) => new TextEncoder().encode(value).length <= WORKBENCH_MAXIMUM_DIRECTORY_PATH_BYTES,
		'Directory path exceeds the UTF-8 byte limit',
	)
	.brand<'WorkbenchDirectoryPath'>();

export type SiteId = z.infer<typeof SiteIdSchema>;
export type WorkbenchRequestId = z.infer<typeof WorkbenchRequestIdSchema>;
export type WorkbenchProjectId = z.infer<typeof WorkbenchProjectIdSchema>;
export type WorkbenchWorkspaceId = z.infer<typeof WorkbenchWorkspaceIdSchema>;
export type WorkbenchDirectoryId = z.infer<typeof WorkbenchDirectoryIdSchema>;
export type WorkbenchDirectoryPath = z.infer<typeof WorkbenchDirectoryPathSchema>;
export type WorkbenchTerminalContextId = z.infer<typeof WorkbenchTerminalContextIdSchema>;
export type WorkbenchTabGroupId = z.infer<typeof WorkbenchTabGroupIdSchema>;
export type WorkbenchTabId = z.infer<typeof WorkbenchTabIdSchema>;
export type WorkbenchPanelId = z.infer<typeof WorkbenchPanelIdSchema>;
export type WorkbenchSplitId = z.infer<typeof WorkbenchSplitIdSchema>;
export type WorkbenchPanelSlotId = z.infer<typeof WorkbenchPanelSlotIdSchema>;
export type TerminalPresentationId = z.infer<typeof TerminalPresentationIdSchema>;
export type WorkerId = z.infer<typeof WorkerIdSchema>;
export type TerminalId = z.infer<typeof TerminalIdSchema>;
export type WorkbenchRevision = z.infer<typeof WorkbenchRevisionSchema>;

export const WorkbenchPresentationBindingSchema = z
	.strictObject({ terminalId: TerminalIdSchema, workerId: WorkerIdSchema })
	.readonly();

export const WorkbenchPresentationSchema = z
	.strictObject({
		binding: WorkbenchPresentationBindingSchema.nullable(),
		customLabel: WorkbenchPanelCustomLabelSchema.optional(),
		label: WorkbenchLabelSchema,
		presentationId: TerminalPresentationIdSchema,
	})
	.readonly();

export type WorkbenchPresentationBinding = z.infer<typeof WorkbenchPresentationBindingSchema>;
export type WorkbenchPresentation = z.infer<typeof WorkbenchPresentationSchema>;

export interface WorkbenchPanelNodePanel {
	readonly rightClick?: 'orchbee' | 'panel' | undefined;
	readonly childPresentations: readonly WorkbenchPresentation[];
	readonly kind: 'panel';
	readonly panelId: WorkbenchPanelId;
	readonly primaryPresentation: WorkbenchPresentation;
}

export interface WorkbenchPanelNodeSlot {
	readonly node: WorkbenchPanelNode;
	readonly slotId: WorkbenchPanelSlotId;
}

export const WorkbenchSplitRatioSchema = z.number().finite().min(0.1).max(0.9);

export interface WorkbenchPanelNodeSplit {
	readonly children: readonly [WorkbenchPanelNodeSlot, WorkbenchPanelNodeSlot];
	readonly kind: 'split';
	readonly orientation: 'horizontal' | 'vertical';
	readonly splitId: WorkbenchSplitId;
	readonly ratio?: number | undefined;
}

export type WorkbenchPanelNode = WorkbenchPanelNodePanel | WorkbenchPanelNodeSplit;

export const WorkbenchPanelNodeSchema: z.ZodType<WorkbenchPanelNode> = z.lazy(() =>
	z.discriminatedUnion('kind', [
		z
			.strictObject({
				childPresentations: z
					.array(WorkbenchPresentationSchema)
					.max(WORKBENCH_MAXIMUM_CHILD_PRESENTATIONS_PER_PANEL)
					.readonly(),
				kind: z.literal('panel'),
				rightClick: z.enum(['orchbee', 'panel']).optional(),
				panelId: WorkbenchPanelIdSchema,
				primaryPresentation: WorkbenchPresentationSchema,
			})
			.readonly(),
		z
			.strictObject({
				children: z
					.tuple([
						z
							.strictObject({
								node: WorkbenchPanelNodeSchema,
								slotId: WorkbenchPanelSlotIdSchema,
							})
							.readonly(),
						z
							.strictObject({
								node: WorkbenchPanelNodeSchema,
								slotId: WorkbenchPanelSlotIdSchema,
							})
							.readonly(),
					])
					.readonly(),
				kind: z.literal('split'),
				orientation: z.enum(['horizontal', 'vertical']),
				splitId: WorkbenchSplitIdSchema,
				ratio: WorkbenchSplitRatioSchema.optional(),
			})
			.readonly(),
	]),
);

export const WorkbenchTabGroupSchema = z
	.strictObject({
		color: z.enum(['blue', 'cyan', 'green', 'grey', 'orange', 'pink', 'purple', 'red', 'yellow']),
		groupId: WorkbenchTabGroupIdSchema,
		label: WorkbenchTabGroupLabelSchema,
	})
	.readonly();

export const WorkbenchTabSchema = z
	.strictObject({
		automaticLabel: z.boolean().optional(),
		groupId: WorkbenchTabGroupIdSchema.nullable(),
		label: WorkbenchLabelSchema,
		pinned: z.boolean(),
		root: WorkbenchPanelNodeSchema,
		tabId: WorkbenchTabIdSchema,
	})
	.readonly();

export const WorkbenchTerminalContextSchema = z
	.strictObject({
		groups: z
			.array(WorkbenchTabGroupSchema)
			.max(WORKBENCH_MAXIMUM_TAB_GROUPS_PER_CONTEXT)
			.readonly(),
		tabs: z.array(WorkbenchTabSchema).max(WORKBENCH_MAXIMUM_TABS_PER_CONTEXT).readonly(),
		terminalContextId: WorkbenchTerminalContextIdSchema,
	})
	.readonly();

export const WorkbenchDirectorySchema = z
	.strictObject({
		directoryId: WorkbenchDirectoryIdSchema,
		workerId: WorkerIdSchema,
		path: WorkbenchDirectoryPathSchema,
	})
	.readonly();

export const WorkbenchWorkspaceSchema = z
	.strictObject({
		label: WorkbenchLabelSchema,
		primaryDirectoryId: WorkbenchDirectoryIdSchema.nullable(),
		terminalContextId: WorkbenchTerminalContextIdSchema,
		workspaceId: WorkbenchWorkspaceIdSchema,
	})
	.readonly();

export const WorkbenchProjectSchema = z
	.strictObject({
		directories: z
			.array(WorkbenchDirectorySchema)
			.max(WORKBENCH_MAXIMUM_DIRECTORIES_PER_PROJECT)
			.readonly(),
		label: WorkbenchLabelSchema,
		projectId: WorkbenchProjectIdSchema,
		workspaces: z
			.array(WorkbenchWorkspaceSchema)
			.max(WORKBENCH_MAXIMUM_WORKSPACES_PER_PROJECT)
			.readonly(),
	})
	.readonly();

export const WorkbenchNavigationSchema = z
	.strictObject({
		projects: z.array(WorkbenchProjectSchema).max(WORKBENCH_MAXIMUM_PROJECTS).readonly(),
		standaloneTerminalContextId: WorkbenchTerminalContextIdSchema,
		workspaceOrder: z
			.array(WorkbenchWorkspaceIdSchema)
			.max(WORKBENCH_MAXIMUM_TERMINAL_CONTEXTS - 1)
			.readonly()
			.optional(),
	})
	.superRefine((navigation, context) => {
		const order = navigation.workspaceOrder;
		if (order === undefined || order.length === 0) return;
		const members = navigation.projects.flatMap((project) =>
			project.workspaces.map((workspace) => workspace.workspaceId),
		);
		const identities = new Set(members);
		if (
			order.length !== members.length ||
			identities.size !== members.length ||
			new Set(order).size !== order.length ||
			order.some((id) => !identities.has(id))
		) {
			context.addIssue({
				code: 'custom',
				message: 'Workspace display order must be an exact permutation of navigation membership',
			});
		}
	})
	.readonly();

const workbenchSnapshotShapeSchema = z.strictObject({
	kind: z.literal('workbench.snapshot'),
	navigation: WorkbenchNavigationSchema,
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	revision: WorkbenchRevisionSchema,
	siteId: SiteIdSchema,
	terminalContexts: z
		.array(WorkbenchTerminalContextSchema)
		.min(1)
		.max(WORKBENCH_MAXIMUM_TERMINAL_CONTEXTS)
		.readonly(),
});

type WorkbenchSnapshotShape = z.infer<typeof workbenchSnapshotShapeSchema>;

const workbenchSnapshotBaseSchema: z.ZodType<WorkbenchSnapshotShape> = workbenchSnapshotShapeSchema
	.superRefine(validateWorkbenchSnapshot)
	.readonly();

export const WorkbenchSnapshotSchema = z.preprocess(
	(value) => (hasExcessiveDecodedDepth(value) ? excessiveDepthSentinel : value),
	workbenchSnapshotBaseSchema,
);

export const WorkbenchSnapshotReadIntentSchema = z
	.strictObject({
		kind: z.literal('workbench.snapshot.read'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
	})
	.readonly();

const WorkbenchDeltaChangeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({ kind: z.literal('navigation-replaced'), navigation: WorkbenchNavigationSchema })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('terminal-context-upserted'),
			terminalContext: WorkbenchTerminalContextSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('terminal-context-removed'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
		})
		.readonly(),
]);

const workbenchDeltaBaseSchema = z
	.strictObject({
		baseRevision: WorkbenchRevisionSchema,
		changes: z.array(WorkbenchDeltaChangeSchema).max(WORKBENCH_MAXIMUM_DELTA_CHANGES).readonly(),
		kind: z.literal('workbench.delta'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		revision: WorkbenchRevisionSchema,
		siteId: SiteIdSchema,
	})
	.readonly();

export const WorkbenchDeltaSchema = z.preprocess(
	(value) => (hasExcessiveDecodedDepth(value) ? excessiveDepthSentinel : value),
	workbenchDeltaBaseSchema,
);

export const WorkbenchProtocolDocumentSchema = z.union([
	WorkbenchSnapshotReadIntentSchema,
	WorkbenchSnapshotSchema,
	WorkbenchDeltaSchema,
]);

export type WorkbenchTabGroup = z.infer<typeof WorkbenchTabGroupSchema>;
export type WorkbenchTab = z.infer<typeof WorkbenchTabSchema>;
export type WorkbenchTerminalContext = z.infer<typeof WorkbenchTerminalContextSchema>;
export type WorkbenchDirectory = z.infer<typeof WorkbenchDirectorySchema>;
export type WorkbenchWorkspace = z.infer<typeof WorkbenchWorkspaceSchema>;
export type WorkbenchProject = z.infer<typeof WorkbenchProjectSchema>;
export type WorkbenchNavigation = z.infer<typeof WorkbenchNavigationSchema>;
export type WorkbenchSnapshot = z.infer<typeof WorkbenchSnapshotSchema>;
export type WorkbenchSnapshotReadIntent = z.infer<typeof WorkbenchSnapshotReadIntentSchema>;
export type WorkbenchDeltaChange = z.infer<typeof WorkbenchDeltaChangeSchema>;
export type WorkbenchDelta = z.infer<typeof WorkbenchDeltaSchema>;
export type WorkbenchProtocolDocument = z.infer<typeof WorkbenchProtocolDocumentSchema>;

export type WorkbenchDeltaRejectionReason =
	| 'base-revision-mismatch'
	| 'duplicate-change-target'
	| 'invalid-projection'
	| 'non-increasing-revision'
	| 'site-mismatch';

export type WorkbenchDeltaApplicationResult =
	| { readonly kind: 'applied'; readonly snapshot: WorkbenchSnapshot }
	| { readonly kind: 'rejected'; readonly reason: WorkbenchDeltaRejectionReason };

export function parseWorkbenchSnapshotReadIntent(value: unknown): WorkbenchSnapshotReadIntent {
	return WorkbenchSnapshotReadIntentSchema.parse(value);
}

export function parseWorkbenchSnapshot(value: unknown): WorkbenchSnapshot {
	return WorkbenchSnapshotSchema.parse(value);
}

export function parseWorkbenchDelta(value: unknown): WorkbenchDelta {
	return WorkbenchDeltaSchema.parse(value);
}

export function parseWorkbenchProtocolDocument(value: unknown): WorkbenchProtocolDocument {
	return WorkbenchProtocolDocumentSchema.parse(value);
}

export function applyWorkbenchDelta(
	snapshot: WorkbenchSnapshot,
	delta: WorkbenchDelta,
): WorkbenchDeltaApplicationResult {
	if (snapshot.siteId !== delta.siteId) return { kind: 'rejected', reason: 'site-mismatch' };
	if (snapshot.revision !== delta.baseRevision) {
		return { kind: 'rejected', reason: 'base-revision-mismatch' };
	}
	if (BigInt(delta.revision) <= BigInt(delta.baseRevision)) {
		return { kind: 'rejected', reason: 'non-increasing-revision' };
	}

	let navigation = snapshot.navigation;
	let navigationReplaced = false;
	const changedContextIds = new Set<WorkbenchTerminalContextId>();
	const terminalContexts = new Map(
		snapshot.terminalContexts.map((terminalContext) => [
			terminalContext.terminalContextId,
			terminalContext,
		]),
	);

	for (const change of delta.changes) {
		if (change.kind === 'navigation-replaced') {
			if (navigationReplaced) return { kind: 'rejected', reason: 'duplicate-change-target' };
			navigationReplaced = true;
			navigation = change.navigation;
			continue;
		}

		const terminalContextId =
			change.kind === 'terminal-context-upserted'
				? change.terminalContext.terminalContextId
				: change.terminalContextId;
		if (changedContextIds.has(terminalContextId)) {
			return { kind: 'rejected', reason: 'duplicate-change-target' };
		}
		changedContextIds.add(terminalContextId);
		if (change.kind === 'terminal-context-upserted') {
			terminalContexts.set(terminalContextId, change.terminalContext);
		} else {
			terminalContexts.delete(terminalContextId);
		}
	}

	const candidate = WorkbenchSnapshotSchema.safeParse({
		kind: 'workbench.snapshot',
		navigation,
		protocolVersion: WORKBENCH_PROTOCOL_VERSION,
		revision: delta.revision,
		siteId: snapshot.siteId,
		terminalContexts: [...terminalContexts.values()].sort((left, right) =>
			compareIdentity(left.terminalContextId, right.terminalContextId),
		),
	});
	return candidate.success
		? { kind: 'applied', snapshot: candidate.data }
		: { kind: 'rejected', reason: 'invalid-projection' };
}

interface SnapshotValidationIdentities {
	readonly boundTerminalIds: Set<TerminalId>;
	readonly groupIds: Set<WorkbenchTabGroupId>;
	readonly panelIds: Set<WorkbenchPanelId>;
	readonly presentationIds: Set<TerminalPresentationId>;
	readonly slotIds: Set<WorkbenchPanelSlotId>;
	readonly splitIds: Set<WorkbenchSplitId>;
	readonly tabIds: Set<WorkbenchTabId>;
}

interface ContextTreeValidationState {
	nodeCount: number;
}

function validateWorkbenchSnapshot(
	snapshot: WorkbenchSnapshotShape,
	context: z.RefinementCtx,
): void {
	validateCanonicalOrder(
		snapshot.navigation.projects.map((project) => project.projectId),
		'Projects',
		context,
	);
	const workspaceIds = new Set<WorkbenchWorkspaceId>();
	const directoryIds = new Set<WorkbenchDirectoryId>();
	const referencedContextCounts = new Map<WorkbenchTerminalContextId, number>();
	incrementReference(referencedContextCounts, snapshot.navigation.standaloneTerminalContextId);
	for (const project of snapshot.navigation.projects) {
		validateCanonicalOrder(
			project.directories.map((directory) => directory.directoryId),
			`Directories in Project ${project.projectId}`,
			context,
		);
		const projectDirectoryIds = new Set<WorkbenchDirectoryId>();
		for (const directory of project.directories) {
			requireUnique(projectDirectoryIds, directory.directoryId, 'Directory in Project', context);
			requireUnique(directoryIds, directory.directoryId, 'Directory', context);
		}
		validateCanonicalOrder(
			project.workspaces.map((workspace) => workspace.workspaceId),
			`Workspaces in Project ${project.projectId}`,
			context,
		);
		for (const workspace of project.workspaces) {
			requireUnique(workspaceIds, workspace.workspaceId, 'Workspace', context);
			incrementReference(referencedContextCounts, workspace.terminalContextId);
			if (
				workspace.primaryDirectoryId !== null &&
				!projectDirectoryIds.has(workspace.primaryDirectoryId)
			) {
				context.addIssue({
					code: 'custom',
					message: `Workspace ${workspace.workspaceId} references a Directory outside its Project`,
				});
			}
		}
	}

	validateCanonicalOrder(
		snapshot.terminalContexts.map((terminalContext) => terminalContext.terminalContextId),
		'Terminal Contexts',
		context,
	);
	const contextIds = new Set(
		snapshot.terminalContexts.map((terminalContext) => terminalContext.terminalContextId),
	);
	for (const [terminalContextId, count] of referencedContextCounts) {
		if (!contextIds.has(terminalContextId)) {
			context.addIssue({
				code: 'custom',
				message: `Missing referenced Terminal Context ${terminalContextId}`,
			});
		}
		if (count !== 1) {
			context.addIssue({
				code: 'custom',
				message: `Terminal Context ${terminalContextId} is referenced ${count} times`,
			});
		}
	}
	for (const terminalContextId of contextIds) {
		if (!referencedContextCounts.has(terminalContextId)) {
			context.addIssue({
				code: 'custom',
				message: `Unreferenced Terminal Context ${terminalContextId}`,
			});
		}
	}

	const identities: SnapshotValidationIdentities = {
		boundTerminalIds: new Set(),
		groupIds: new Set(),
		panelIds: new Set(),
		presentationIds: new Set(),
		slotIds: new Set(),
		splitIds: new Set(),
		tabIds: new Set(),
	};
	for (const terminalContext of snapshot.terminalContexts) {
		validateTerminalContext(terminalContext, identities, context);
	}
}

function validateTerminalContext(
	terminalContext: WorkbenchTerminalContext,
	identities: SnapshotValidationIdentities,
	context: z.RefinementCtx,
): void {
	const localGroupIds = new Set<WorkbenchTabGroupId>();
	for (const group of terminalContext.groups) {
		requireUnique(localGroupIds, group.groupId, 'Tab Group in Terminal Context', context);
		requireUnique(identities.groupIds, group.groupId, 'Tab Group', context);
	}

	let foundUnpinnedTab = false;
	const groupPositions = new Map<WorkbenchTabGroupId, number[]>();
	const treeState: ContextTreeValidationState = { nodeCount: 0 };
	for (const [index, tab] of terminalContext.tabs.entries()) {
		requireUnique(identities.tabIds, tab.tabId, 'Tab', context);
		if (tab.pinned) {
			if (foundUnpinnedTab) {
				context.addIssue({ code: 'custom', message: 'Pinned Tabs must form one leading prefix' });
			}
			if (tab.groupId !== null) {
				context.addIssue({ code: 'custom', message: `Pinned Tab ${tab.tabId} cannot be grouped` });
			}
		} else {
			foundUnpinnedTab = true;
		}
		if (tab.groupId !== null) {
			if (!localGroupIds.has(tab.groupId)) {
				context.addIssue({
					code: 'custom',
					message: `Tab ${tab.tabId} references a missing Tab Group`,
				});
			}
			const positions = groupPositions.get(tab.groupId) ?? [];
			positions.push(index);
			groupPositions.set(tab.groupId, positions);
		}
		validatePanelNode(tab.root, 1, treeState, identities, context);
	}
	for (const [groupId, positions] of groupPositions) {
		const first = positions[0];
		const last = positions.at(-1);
		if (first === undefined || last === undefined || last - first + 1 !== positions.length) {
			context.addIssue({
				code: 'custom',
				message: `Tab Group ${groupId} members must be contiguous`,
			});
		}
	}
	if (treeState.nodeCount > WORKBENCH_MAXIMUM_PANEL_NODES_PER_CONTEXT) {
		context.addIssue({
			code: 'custom',
			message: `Terminal Context ${terminalContext.terminalContextId} exceeds the Panel/Split node limit`,
		});
	}
}

function validatePanelNode(
	node: WorkbenchPanelNode,
	depth: number,
	state: ContextTreeValidationState,
	identities: SnapshotValidationIdentities,
	context: z.RefinementCtx,
): void {
	state.nodeCount += 1;
	if (depth > WORKBENCH_MAXIMUM_PANEL_NODE_DEPTH) {
		context.addIssue({ code: 'custom', message: 'Panel/Split tree exceeds the depth limit' });
	}
	if (node.kind === 'panel') {
		requireUnique(identities.panelIds, node.panelId, 'Panel', context);
		validatePresentation(node.primaryPresentation, identities, context);
		for (const presentation of node.childPresentations) {
			validatePresentation(presentation, identities, context);
		}
		return;
	}
	requireUnique(identities.splitIds, node.splitId, 'Split', context);
	for (const child of node.children) {
		requireUnique(identities.slotIds, child.slotId, 'Panel Slot', context);
		validatePanelNode(child.node, depth + 1, state, identities, context);
	}
}

function validatePresentation(
	presentation: WorkbenchPresentation,
	identities: SnapshotValidationIdentities,
	context: z.RefinementCtx,
): void {
	requireUnique(
		identities.presentationIds,
		presentation.presentationId,
		'Terminal Presentation',
		context,
	);
	if (presentation.binding !== null) {
		requireUnique(
			identities.boundTerminalIds,
			presentation.binding.terminalId,
			'bound Terminal',
			context,
		);
	}
}

function validateCanonicalOrder(
	identities: readonly string[],
	domain: string,
	context: z.RefinementCtx,
): void {
	let previous: string | undefined;
	for (const identity of identities) {
		if (previous !== undefined && compareIdentity(previous, identity) >= 0) {
			context.addIssue({
				code: 'custom',
				message: `${domain} must use unique canonical identity order`,
			});
			return;
		}
		previous = identity;
	}
}

function compareIdentity(left: string, right: string): number {
	if (left < right) return -1;
	return left === right ? 0 : 1;
}

function requireUnique<Identity extends string>(
	identities: Set<Identity>,
	identity: Identity,
	domain: string,
	context: z.RefinementCtx,
): void {
	if (identities.has(identity)) {
		context.addIssue({ code: 'custom', message: `Duplicate ${domain} identity ${identity}` });
		return;
	}
	identities.add(identity);
}

function incrementReference<Identity extends string>(
	counts: Map<Identity, number>,
	identity: Identity,
): void {
	counts.set(identity, (counts.get(identity) ?? 0) + 1);
}

function hasExcessiveDecodedDepth(value: unknown): boolean {
	const pending: { readonly depth: number; readonly value: unknown }[] = [{ depth: 0, value }];
	while (pending.length > 0) {
		const current = pending.pop();
		if (current === undefined) break;
		if (current.depth > MAXIMUM_DECODED_VALUE_DEPTH) return true;
		if (current.value === null || typeof current.value !== 'object') continue;
		for (const child of Object.values(current.value)) {
			pending.push({ depth: current.depth + 1, value: child });
		}
	}
	return false;
}
