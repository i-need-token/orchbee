/**
 * Canonical Agent conformance corpus generator.
 *
 * Executes the owning `packages/agent` build output (dist) and emits the durable
 * fixture at schemas/agent/v1/agent-canonical-conformance-v1.json. Expectations are
 * always executed output — never hand-edited.
 *
 *   node conformance/generate-corpus.mjs           # rewrite the fixture
 *   node conformance/generate-corpus.mjs --check   # regenerate in memory, byte-compare, exit 1 on drift
 *   node conformance/generate-corpus.mjs --base <commit>   # pin provenance.baseCommit
 *
 * Run `pnpm -F @orchbee/agent build` first so dist matches the fingerprinted sources.
 */
import { execSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
	AgentRuntime,
	explainAgentSession,
	matchingAgentWaitConditions,
	projectAgentLocation,
	projectAgentSessionGroup,
	projectAgentSessionSummary,
} from '../dist/index.js';

const require = createRequire(import.meta.url);
const conformanceDir = dirname(fileURLToPath(import.meta.url));
const packageDir = join(conformanceDir, '..');
const repoRoot = join(packageDir, '..', '..');
const OUTPUT = join(repoRoot, 'schemas', 'agent', 'v1', 'agent-canonical-conformance-v1.json');
const CHECK_MODE = process.argv.includes('--check');
const baseArgIndex = process.argv.indexOf('--base');
const baseCommit =
	baseArgIndex >= 0
		? process.argv[baseArgIndex + 1]
		: execSync('git rev-parse HEAD', { cwd: repoRoot, encoding: 'utf8' }).trim();

function collectSourceFingerprints() {
	const srcDir = join(packageDir, 'src');
	const entries = [];
	const walk = (dir) => {
		const byName = (a, b) => {
			if (a.name === b.name) return 0;
			return a.name < b.name ? -1 : 1;
		};
		for (const entry of readdirSync(dir, { withFileTypes: true }).sort(byName)) {
			const path = join(dir, entry.name);
			if (entry.isDirectory()) walk(path);
			else if (entry.name.endsWith('.ts')) entries.push(path);
		}
	};
	walk(srcDir);
	return entries.sort().map((path) => ({
		path: relative(srcDir, path).split('\\').join('/'),
		sha256: createHash('sha256').update(readFileSync(path)).digest('hex'),
	}));
}

const ALL_WAIT_CONDITIONS = [
	'needs-attention',
	'failed',
	'succeeded',
	'canceled',
	'interrupted',
	'working',
	'idle',
	'unknown',
	'ended',
	'fresh',
	'stale',
	'disconnected',
];

const CAPS_FULL = [
	'sessionLifecycle',
	'turnLifecycle',
	'questions',
	'approvals',
	'authentication',
	'activity',
	'subagents',
	'progress',
];

// ---- event builders (plain JSON; branded types are runtime-plain strings) ----

let eventCounter = 0;
const eid = () => `evt-${(eventCounter += 1)}`;

const obs = (sessionId, sequence, over = {}) => ({
	capabilities: [...CAPS_FULL],
	eventId: eid(),
	executionLocation: { executionLocationId: `exec-${sessionId}` },
	observation: {
		fidelity: 'rich',
		freshness: 'fresh',
		generation: 0,
		sequence,
		source: 'hook',
	},
	parentSessionId: null,
	presentationAttachments: [],
	providerId: 'claude-code',
	sessionId,
	type: 'session-observed',
	...over,
});

const ev = (sessionId, generation, sequence, type, over = {}) => ({
	eventId: eid(),
	generation,
	sequence,
	sessionId,
	type,
	...over,
});

const attachment = (presentationId, generation, over = {}) => ({
	generation,
	navigable: true,
	presentationId,
	visibility: 'visible',
	...over,
});

const request = (requestId, kind, turnId = null) => ({
	actionPath: 'navigate-only',
	kind,
	requestId,
	turnId,
});

// ---- vector definitions ----

const VECTORS = [
	{
		id: 'v01-unknown-fresh',
		purpose: 'Baseline: observed session with no turn signal projects status unknown.',
		covers: ['status:unknown', 'projection', 'explain'],
		sourceRefs: [
			'packages/agent/src/reducer.ts:110-174',
			'packages/agent/src/projection.ts:197-213',
		],
		assumptions: ['hook source admits the full capability set'],
		events: [obs('sess-main', 1)],
	},
	{
		id: 'v02-working-turn',
		purpose: 'turn-started produces working status carrying activity and phase.',
		covers: ['status:working', 'turn'],
		sourceRefs: ['packages/agent/src/reducer.ts:176-201'],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
		],
	},
	{
		id: 'v03-turn-phases',
		purpose: 'queued phase at start; turn-activity-changed moves to waiting-external.',
		covers: ['status:working', 'turn', 'phase-transitions'],
		sourceRefs: ['packages/agent/src/reducer.ts:221-237'],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'turn-started', {
				activity: null,
				phase: 'queued',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 3, 'turn-activity-changed', {
				activity: 'tool',
				phase: 'waiting-external',
				turnId: 'turn-1',
			}),
		],
	},
	{
		id: 'v04-idle-and-repeat-applies',
		purpose:
			'session-idle-observed yields idle; a repeated idle observation still applies a revision (no state-equality dedup).',
		covers: ['status:idle', 'journal-noise'],
		sourceRefs: ['packages/agent/src/reducer.ts:352-370'],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 3, 'session-idle-observed'),
			ev('sess-main', 0, 4, 'session-idle-observed'),
		],
		expectedNotes: [
			'step 3 is applied (revision advances) even though the visible state is identical; the change journal records it',
		],
	},
	{
		id: 'v05-attention-request-resolve',
		purpose:
			'Unscoped question request produces needs-attention{count:1}; resolution returns to prior status.',
		covers: ['status:needs-attention', 'attention', 'semantic-events'],
		sourceRefs: ['packages/agent/src/reducer.ts:239-317'],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'attention-requested', { request: request('req-q1', 'question') }),
			ev('sess-main', 0, 3, 'attention-resolved', { requestId: 'req-q1' }),
		],
	},
	{
		id: 'v06-turn-scoped-request-autoclear',
		purpose:
			'A turn-scoped request is silently dropped by turn-finished: only the turn-outcome semantic event is emitted, never attention-resolved.',
		covers: ['attention', 'semantic-events', 'status:succeeded'],
		sourceRefs: ['packages/agent/src/reducer.ts:319-350'],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'turn-started', {
				activity: 'tool',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 3, 'attention-requested', {
				request: request('req-a1', 'approval', 'turn-1'),
			}),
			ev('sess-main', 0, 4, 'turn-finished', { outcome: 'succeeded', turnId: 'turn-1' }),
			ev('sess-main', 0, 5, 'attention-resolved', { requestId: 'req-a1' }),
		],
		expectedNotes: [
			'step 4 semanticEvents contain only turn-outcome; the dropped request emits no attention-resolved',
			'step 5 is rejected request-not-found because the request was already cleared',
		],
	},
	{
		id: 'v07-turn-outcomes',
		purpose: 'Each turn outcome projects its own terminal status.',
		covers: ['status:succeeded', 'status:failed', 'status:canceled', 'status:interrupted', 'turn'],
		sourceRefs: [
			'packages/agent/src/reducer.ts:319-350',
			'packages/agent/src/projection.ts:197-213',
		],
		assumptions: [],
		events: ['succeeded', 'failed', 'canceled', 'interrupted'].flatMap((outcome, index) => [
			obs(`sess-${outcome}`, 1 + index * 3),
			ev(`sess-${outcome}`, 0, 2 + index * 3, 'turn-started', {
				activity: 'tool',
				phase: 'working',
				turnId: `turn-${outcome}`,
			}),
			ev(`sess-${outcome}`, 0, 3 + index * 3, 'turn-finished', {
				outcome,
				turnId: `turn-${outcome}`,
			}),
		]),
	},
	{
		id: 'v08-ended-after-idle',
		purpose: 'Explicit session-ended on an idle session projects ended with null lastOutcome.',
		covers: ['status:ended', 'session-end'],
		sourceRefs: ['packages/agent/src/reducer.ts:529-588'],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'session-idle-observed'),
			ev('sess-main', 0, 3, 'session-ended'),
		],
	},
	{
		id: 'v09-ended-interrupts-active-turn',
		purpose:
			'session-ended during an active turn records lastOutcome interrupted and emits turn-outcome then session-ended; the projected status is still ended.',
		covers: ['status:ended', 'session-end', 'semantic-events', 'implicit-interruption'],
		sourceRefs: ['packages/agent/src/reducer.ts:529-588'],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'turn-started', {
				activity: 'tool',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 3, 'session-ended'),
		],
		expectedNotes: [
			'lastOutcome.kind is interrupted but resolveStatus returns ended because lifecycle wins over outcome',
		],
	},
	{
		id: 'v10-heuristic-presence-retire',
		purpose:
			'Ending a heuristic-only presence emits zero semantic events and forces freshness disconnected — a silent retire, not an observable end.',
		covers: ['status:ended', 'session-end', 'retire', 'freshness'],
		sourceRefs: ['packages/agent/src/reducer.ts:535-584'],
		assumptions: ['heuristic sessions must carry fidelity=heuristic and zero capabilities'],
		events: [
			obs('sess-heur', 1, {
				capabilities: [],
				observation: {
					fidelity: 'heuristic',
					freshness: 'fresh',
					generation: 0,
					sequence: 1,
					source: 'heuristic',
				},
			}),
			ev('sess-heur', 0, 2, 'session-ended'),
		],
		expectedNotes: [
			"step 2 applies with semanticEvents [] — the session ends silently; wait-matching yields only 'disconnected', never 'ended'",
		],
	},
	{
		id: 'v11-stale-gates-wait',
		purpose:
			'observation-became-stale flips freshness while the projected status stays working; semantic wait conditions stop matching but freshness conditions match.',
		covers: ['freshness', 'wait-gating', 'status:working'],
		sourceRefs: [
			'packages/agent/src/reducer.ts:372-386',
			'packages/agent/src/semantic-commands/agent-semantic-read-model.ts:46-57',
		],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 3, 'observation-became-stale'),
		],
		expectedNotes: [
			"status remains working{...} but matchedWaitConditions is only ['stale'] — semantic conditions require freshness=fresh",
		],
	},
	{
		id: 'v12-any-applied-event-restores-fresh',
		purpose: 'Any applied non-stale transition restores freshness to fresh.',
		covers: ['freshness'],
		sourceRefs: ['packages/agent/src/reducer.ts:640-642'],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 3, 'observation-became-stale'),
			ev('sess-main', 0, 4, 'turn-activity-changed', {
				activity: 'editing',
				phase: 'working',
				turnId: 'turn-1',
			}),
		],
	},
	{
		id: 'v13-last-navigable-detach-disconnects',
		purpose:
			'Detaching the last navigable attachment forces freshness disconnected; a repeat detach of the retired attachment is unchanged.',
		covers: ['freshness', 'presentation', 'disconnected'],
		sourceRefs: ['packages/agent/src/reducer.ts:497-527'],
		assumptions: [],
		events: [
			obs('sess-main', 1, {
				presentationAttachments: [attachment('pres-a', 'pgen-1')],
			}),
			ev('sess-main', 0, 2, 'presentation-detached', {
				presentationGeneration: 'pgen-1',
				presentationId: 'pres-a',
			}),
			ev('sess-main', 0, 3, 'presentation-detached', {
				presentationGeneration: 'pgen-1',
				presentationId: 'pres-a',
			}),
		],
		expectedNotes: [
			'after step 2 the attachment remains but is navigable:false with visibility forced to unknown',
		],
	},
	{
		id: 'v14-reconnect-resync-and-generation-fences',
		purpose:
			'observation-reconnected with a higher generation resynchronizes the whole session claim; same-generation resync is rejected generation-mismatch; higher-generation but stale-sequence resync is unchanged.',
		covers: ['freshness', 'reconnect', 'generation-fence', 'sequence-fence'],
		sourceRefs: ['packages/agent/src/reducer.ts:69-77,388-425'],
		assumptions: [],
		events: [
			obs('sess-main', 1, {
				presentationAttachments: [attachment('pres-a', 'pgen-1')],
			}),
			ev('sess-main', 0, 2, 'presentation-detached', {
				presentationGeneration: 'pgen-1',
				presentationId: 'pres-a',
			}),
			{
				activityHint: null,
				capabilities: [...CAPS_FULL],
				eventId: eid(),
				generation: 1,
				lastOutcome: null,
				pendingRequests: [],
				sequence: 3,
				sessionId: 'sess-main',
				turn: { kind: 'idle' },
				type: 'observation-reconnected',
			},
			{
				activityHint: null,
				capabilities: [...CAPS_FULL],
				eventId: eid(),
				generation: 1,
				lastOutcome: null,
				pendingRequests: [],
				sequence: 4,
				sessionId: 'sess-main',
				turn: { kind: 'idle' },
				type: 'observation-reconnected',
			},
			{
				activityHint: null,
				capabilities: [...CAPS_FULL],
				eventId: eid(),
				generation: 2,
				lastOutcome: null,
				pendingRequests: [],
				sequence: 2,
				sessionId: 'sess-main',
				turn: { kind: 'idle' },
				type: 'observation-reconnected',
			},
		],
		expectedNotes: [
			'step 4 is rejected generation-mismatch (same-generation resync is not a legal event)',
			'step 5 is unchanged: generation wins the fence, sequence dedup still applies inside reconnect',
		],
	},
	{
		id: 'v15-stale-on-disconnected-and-ended',
		purpose:
			'observation-became-stale is unchanged on disconnected sessions but still applies on ended sessions — post-end freshness mutation is legal.',
		covers: ['freshness', 'session-end', 'wait-gating', 'quirk'],
		sourceRefs: ['packages/agent/src/reducer.ts:372-386'],
		assumptions: [],
		events: [
			obs('sess-disc', 1, {
				presentationAttachments: [attachment('pres-a', 'pgen-1')],
			}),
			ev('sess-disc', 0, 2, 'presentation-detached', {
				presentationGeneration: 'pgen-1',
				presentationId: 'pres-a',
			}),
			ev('sess-disc', 0, 3, 'observation-became-stale'),
			obs('sess-dead', 4),
			ev('sess-dead', 0, 5, 'session-ended'),
			ev('sess-dead', 0, 6, 'observation-became-stale'),
		],
		expectedNotes: [
			'step 3 is unchanged (disconnected is terminal for freshness)',
			"step 6 applies: the ended session goes stale, and 'ended' stops matching wait conditions (freshness gate) — only 'stale' matches",
		],
	},
	{
		id: 'v16-sequence-dedup-and-generation-fence',
		purpose:
			'Duplicate and older-sequence events are silently unchanged; wrong-generation events are rejected; unknown sessions are rejected.',
		covers: ['dedup', 'sequence-fence', 'generation-fence', 'rejection'],
		sourceRefs: ['packages/agent/src/reducer.ts:62-78'],
		assumptions: ['sequence need not be contiguous — only strictly increasing'],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 5, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 5, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 3, 'turn-activity-changed', {
				activity: 'tool',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 9, 6, 'observation-became-stale'),
			ev('sess-ghost', 0, 1, 'session-idle-observed'),
		],
		expectedNotes: [
			'step 3 (resent seq 5) and step 4 (seq 3 < 5) are both unchanged, not rejected',
			'sequence gap 1→5 at step 2 is legal',
		],
	},
	{
		id: 'v17-reobserve-idempotency',
		purpose:
			'Re-observing the same identity is unchanged even when capabilities differ — only providerId/executionLocation/attachments define identity conflict.',
		covers: ['identity', 'session-observed', 'quirk'],
		sourceRefs: ['packages/agent/src/reducer.ts:122-133'],
		assumptions: [],
		events: [
			obs('sess-main', 1, { capabilities: ['turnLifecycle'] }),
			obs('sess-main', 2, { capabilities: ['turnLifecycle', 'questions'] }),
			obs('sess-main', 3, {
				executionLocation: { executionLocationId: 'exec-other' },
			}),
		],
		expectedNotes: [
			'step 2 is unchanged: the widened capability set is silently dropped — capabilities can only change via observation-reconnected',
			'step 3 is rejected identity-conflict',
		],
	},
	{
		id: 'v18-turn-conflicts-and-late-events',
		purpose:
			'Concurrent turn conflict, same-turn restart dedup, wrong-turn finish, and a late activity event after finish are all rejected or absorbed.',
		covers: ['turn', 'rejection', 'late-event-after-finish'],
		sourceRefs: ['packages/agent/src/reducer.ts:176-237,319-350'],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 3, 'turn-started', {
				activity: 'tool',
				phase: 'working',
				turnId: 'turn-2',
			}),
			ev('sess-main', 0, 4, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 5, 'turn-finished', { outcome: 'failed', turnId: 'turn-2' }),
			ev('sess-main', 0, 6, 'turn-finished', { outcome: 'failed', turnId: 'turn-1' }),
			ev('sess-main', 0, 7, 'turn-activity-changed', {
				activity: 'tool',
				phase: 'waiting-external',
				turnId: 'turn-1',
			}),
		],
		expectedNotes: [
			'step 3 turn-conflict; step 4 unchanged (same active turnId); step 5 turn-not-found; step 7 late activity on finished turn is turn-not-found',
		],
	},
	{
		id: 'v19-new-turn-clears-outcome',
		purpose:
			'A new turn-started after a finished turn clears lastOutcome — the status returns to working.',
		covers: ['turn', 'status:working', 'quirk'],
		sourceRefs: ['packages/agent/src/reducer.ts:189-200'],
		assumptions: [],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-main', 0, 3, 'turn-finished', { outcome: 'succeeded', turnId: 'turn-1' }),
			ev('sess-main', 0, 4, 'turn-started', {
				activity: null,
				phase: 'queued',
				turnId: 'turn-2',
			}),
		],
	},
	{
		id: 'v20-capability-and-source-gating',
		purpose:
			'Attention kinds require exact capabilities (unclassified accepts any attention capability); limited activity requires terminal source; source-constrained capabilities are enforced at observation.',
		covers: ['capability', 'rejection', 'activityHint', 'source-rules'],
		sourceRefs: ['packages/agent/src/reducer.ts:203-219,652-711'],
		assumptions: [],
		events: [
			obs('sess-a', 1, { capabilities: ['questions'] }),
			ev('sess-a', 0, 2, 'attention-requested', { request: request('req-1', 'question') }),
			ev('sess-a', 0, 3, 'attention-requested', { request: request('req-2', 'approval') }),
			ev('sess-a', 0, 4, 'attention-requested', { request: request('req-3', 'unclassified') }),
			obs('sess-b', 5, { capabilities: [] }),
			ev('sess-b', 0, 6, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-b',
			}),
			obs('sess-c', 7, { capabilities: ['activity'] }),
			ev('sess-c', 0, 8, 'limited-activity-observed', { activity: 'command' }),
			obs('sess-d', 9, {
				capabilities: ['activity'],
				observation: {
					fidelity: 'basic',
					freshness: 'fresh',
					generation: 0,
					sequence: 9,
					source: 'terminal',
				},
			}),
			ev('sess-d', 0, 10, 'limited-activity-observed', { activity: 'command' }),
			obs('sess-e', 11, {
				capabilities: ['turnLifecycle'],
				observation: {
					fidelity: 'basic',
					freshness: 'fresh',
					generation: 0,
					sequence: 11,
					source: 'terminal',
				},
			}),
			obs('sess-f', 12, {
				capabilities: ['turnLifecycle'],
				observation: {
					fidelity: 'heuristic',
					freshness: 'fresh',
					generation: 0,
					sequence: 12,
					source: 'heuristic',
				},
			}),
		],
		expectedNotes: [
			'step 3 rejected capability-not-declared (approval needs approvals); step 4 applied (unclassified accepts any attention capability)',
			'step 6 rejected capability-not-declared (no turnLifecycle)',
			'step 8 rejected source-capability-exceeded (limited activity needs terminal source)',
			'step 10 applied — status unknown{activityHint:command}',
			'step 11 rejected source-capability-exceeded (terminal source allows only activity capability)',
			'step 12 rejected source-capability-exceeded (heuristic source forbids all capabilities)',
		],
	},
	{
		id: 'v21-parentage',
		purpose:
			'Parent must exist in the same snapshot and the child must declare subagents; missing parent is invalid-parent, missing capability is capability-not-declared.',
		covers: ['parentage', 'subagents', 'rejection'],
		sourceRefs: ['packages/agent/src/reducer.ts:147-155'],
		assumptions: [],
		events: [
			obs('sess-parent', 1),
			obs('sess-child', 2, { parentSessionId: 'sess-parent' }),
			obs('sess-orphan', 3, { parentSessionId: 'sess-missing' }),
			obs('sess-nocap', 4, {
				capabilities: CAPS_FULL.filter((cap) => cap !== 'subagents'),
				parentSessionId: 'sess-parent',
			}),
		],
	},
	{
		id: 'v22-presentation-conflicts',
		purpose:
			'Navigable attachments enforce one generation per presentation across the runtime; non-navigable attach input is invalid; visibility changes require a navigable attachment.',
		covers: ['presentation', 'rejection'],
		sourceRefs: ['packages/agent/src/reducer.ts:427-527,774-787'],
		assumptions: [],
		events: [
			obs('sess-a', 1, { presentationAttachments: [attachment('pres-x', 'g1')] }),
			obs('sess-b', 2, { presentationAttachments: [attachment('pres-x', 'g2')] }),
			ev('sess-a', 0, 3, 'presentation-attached', {
				attachment: attachment('pres-y', 'g1', { navigable: false }),
			}),
			ev('sess-a', 0, 4, 'presentation-attached', {
				attachment: attachment('pres-x', 'g1'),
			}),
			ev('sess-a', 0, 5, 'presentation-visibility-changed', {
				presentationGeneration: 'g1',
				presentationId: 'pres-x',
				visibility: 'hidden',
			}),
			ev('sess-a', 0, 6, 'presentation-visibility-changed', {
				presentationGeneration: 'g1',
				presentationId: 'pres-x',
				visibility: 'hidden',
			}),
			ev('sess-a', 0, 7, 'presentation-detached', {
				presentationGeneration: 'g1',
				presentationId: 'pres-x',
			}),
			ev('sess-a', 0, 8, 'presentation-visibility-changed', {
				presentationGeneration: 'g1',
				presentationId: 'pres-x',
				visibility: 'visible',
			}),
		],
		expectedNotes: [
			'step 2 rejected presentation-generation-mismatch (pres-x already navigable at g1)',
			'step 3 rejected invalid-event (attach input must be navigable)',
			'step 4 unchanged (identical attachment)',
			'step 6 unchanged (same visibility)',
			'step 8 rejected presentation-not-found (detached attachment is not navigable)',
		],
	},
	{
		id: 'v23-journal-paging',
		purpose:
			'Bounded journal reads: contiguous pages, hasMore, current at head, revision-ahead beyond head, and empty-runtime current.',
		covers: ['journal', 'paging', 'cursor-reset'],
		sourceRefs: ['packages/agent/src/changes.ts:151-202'],
		assumptions: [],
		events: [
			obs('sess-a', 1),
			obs('sess-b', 2),
			ev('sess-a', 0, 3, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-a', 0, 4, 'turn-activity-changed', {
				activity: 'tool',
				phase: 'working',
				turnId: 'turn-1',
			}),
			ev('sess-a', 0, 5, 'attention-requested', { request: request('req-1', 'question') }),
			ev('sess-a', 0, 6, 'attention-resolved', { requestId: 'req-1' }),
		],
		journalReads: [
			{ afterRevision: 0, limit: 3 },
			{ afterRevision: 3, limit: 3 },
			{ afterRevision: 6 },
			{ afterRevision: 99 },
			{ afterRevision: 0, onEmptyRuntime: true, note: 'empty runtime reads current, not baseline' },
		],
	},
	{
		id: 'v24-retention-overflow-reset',
		purpose:
			'Past the 512-change retention window a stale afterRevision returns reset-required jumping observedThroughRevision to the runtime head; the first.retained-1 boundary still pages normally.',
		covers: ['journal', 'cursor-reset', 'retention'],
		sourceRefs: ['packages/agent/src/changes.ts:139-202'],
		assumptions: [
			'program vector: template event repeats with sequence = sequenceStart + index and eventId = prefix + index',
		],
		events: [
			obs('sess-main', 1),
			ev('sess-main', 0, 2, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
		],
		program: {
			count: 520,
			eventIdPrefix: 'evt-fill-',
			sequenceStart: 3,
			template: ev('sess-main', 0, 0, 'turn-activity-changed', {
				activity: 'tool',
				phase: 'working',
				turnId: 'turn-1',
			}),
		},
		journalReads: [
			{ afterRevision: 1, note: 'deep-stale cursor → reset-required' },
			{ afterRevision: 9, note: 'two below first retained (11) → reset-required' },
			{ afterRevision: 10, note: 'first retained - 1 → contiguous page from 11' },
			{ afterRevision: 522, note: 'at head → current' },
			{ afterRevision: 600, note: 'beyond head → revision-ahead' },
		],
	},
	{
		id: 'v25-canonical-order',
		purpose:
			'Session, attachment, and request ordering is UTF-16 code-unit order — astral presentation ids sort before BMP ids ≥ U+E000.',
		covers: ['canonical-order', 'utf16-ordering', 'quirk'],
		sourceRefs: [
			'packages/agent/src/model.ts:9-12',
			'packages/agent/src/__tests__/canonical-order-locale.unit.test.ts',
		],
		assumptions: ['sessionIds are ASCII-only; only presentationIds may be non-ASCII'],
		events: [
			...['s:a', 's:_', 's:Z', 's::', 's:-', 's:9'].map((sessionId, index) =>
				obs(sessionId, index + 1),
			),
			obs('s:z', 7, {
				presentationAttachments: [
					attachment('', 'g1', { navigable: false }),
					attachment('😀', 'g2', { navigable: false }),
					attachment('中', 'g3', { navigable: false }),
					attachment('é', 'g4', { navigable: false }),
					attachment('é', 'g5', { navigable: false }),
					attachment('z', 'g6', { navigable: false }),
				],
			}),
			ev('s:a', 0, 8, 'attention-requested', { request: request('req:z', 'unclassified') }),
			ev('s:a', 0, 9, 'attention-requested', { request: request('req:1', 'unclassified') }),
			ev('s:a', 0, 10, 'attention-requested', { request: request('req:A', 'unclassified') }),
		],
		expectedNotes: [
			"session order must be ['s:-','s:9','s::','s:Z','s:_','s:a','s:z']",
			"attachment order must be ['e\\u0301','z','é','中','😀',''] — the astral '😀' precedes U+E000 because its UTF-16 lead surrogate D83D < E000",
			"request order must be ['req:1','req:A','req:z']",
		],
	},
	{
		id: 'v26-aggregate-projection',
		purpose:
			'Aggregate status follows priority needs-attention > failed|interrupted|canceled > succeeded > working > idle > unknown > ended; live sessions displace ended ones in groups; per-location projection groups by attachment.',
		covers: ['aggregate', 'projection', 'status-priority'],
		sourceRefs: ['packages/agent/src/projection.ts:132-275'],
		assumptions: [],
		events: [
			obs('sess-attn', 1, { presentationAttachments: [attachment('pres-x', 'g1')] }),
			ev('sess-attn', 0, 2, 'attention-requested', { request: request('req-1', 'question') }),
			ev('sess-attn', 0, 3, 'attention-requested', { request: request('req-2', 'approval') }),
			obs('sess-work', 4, { presentationAttachments: [attachment('pres-x', 'g1')] }),
			ev('sess-work', 0, 5, 'turn-started', {
				activity: 'tool',
				phase: 'working',
				turnId: 'turn-w',
			}),
			obs('sess-idle', 6),
			ev('sess-idle', 0, 7, 'session-idle-observed'),
			obs('sess-dead', 8),
			ev('sess-dead', 0, 9, 'session-ended'),
		],
		aggregateSessionIds: ['sess-attn', 'sess-work', 'sess-idle', 'sess-dead'],
		locationPresentationId: 'pres-x',
		expectedNotes: [
			'aggregate status needs-attention{count:2}; aggregate freshness is computed across the attention-bearing sessions only',
			'projectAgentSessionGroup excludes ended sessions whenever any live session exists (sess-dead drops out)',
			'location pres-x aggregates only sess-attn + sess-work → needs-attention{count:2}',
		],
	},
	{
		id: 'v27-post-end-lifecycle',
		purpose:
			'Ended sessions reject work requests but still accept freshness mutations: detach applies and disconnects; re-observing an ended session is unchanged, never a resurrection.',
		covers: ['session-end', 'post-end', 'quirk'],
		sourceRefs: ['packages/agent/src/reducer.ts:245,294,325,433,497-527'],
		assumptions: [],
		events: [
			obs('sess-main', 1, { presentationAttachments: [attachment('pres-a', 'g1')] }),
			ev('sess-main', 0, 2, 'session-ended'),
			ev('sess-main', 0, 3, 'attention-requested', { request: request('req-1', 'question') }),
			ev('sess-main', 0, 4, 'turn-started', {
				activity: 'thinking',
				phase: 'working',
				turnId: 'turn-1',
			}),
			obs('sess-main', 5, { presentationAttachments: [attachment('pres-a', 'g1')] }),
			ev('sess-main', 0, 6, 'presentation-detached', {
				presentationGeneration: 'g1',
				presentationId: 'pres-a',
			}),
			obs('sess-main', 7, { presentationAttachments: [attachment('pres-a', 'g1')] }),
			obs('sess-main', 8, {
				presentationAttachments: [
					attachment('pres-a', 'g1', { navigable: false, visibility: 'unknown' }),
				],
			}),
		],
		expectedNotes: [
			'steps 3-4 rejected session-ended',
			'step 5 unchanged: identical re-observe of an ended session is idempotent, never a resurrection',
			'step 6 applies: detach has no lifecycle guard, and losing the last navigable attachment disconnects even an ended session',
			'step 7 rejected identity-conflict: the post-end detach mutated stored attachments, so the original re-observe no longer matches identity',
			'step 8 unchanged: re-observing with the mutated attachment shape matches the stored identity — identity comparison tracks stored state, not the original claim',
		],
	},
	{
		id: 'v28-reconnect-claim-validation',
		purpose:
			'A resync whose claims exceed the declared capabilities is rejected capability-not-declared; on an ended session reconnect is rejected session-ended.',
		covers: ['reconnect', 'capability', 'rejection'],
		sourceRefs: ['packages/agent/src/reducer.ts:388-425,692-711'],
		assumptions: [],
		events: [
			obs('sess-main', 1, { capabilities: ['turnLifecycle'] }),
			{
				activityHint: null,
				capabilities: ['turnLifecycle'],
				eventId: eid(),
				generation: 1,
				lastOutcome: null,
				pendingRequests: [request('req-1', 'question', 'turn-1')],
				sequence: 2,
				sessionId: 'sess-main',
				turn: { activity: 'tool', kind: 'active', phase: 'working', turnId: 'turn-1' },
				type: 'observation-reconnected',
			},
			obs('sess-dead', 3),
			ev('sess-dead', 0, 4, 'session-ended'),
			{
				activityHint: null,
				capabilities: [...CAPS_FULL],
				eventId: eid(),
				generation: 1,
				lastOutcome: null,
				pendingRequests: [],
				sequence: 5,
				sessionId: 'sess-dead',
				turn: { kind: 'idle' },
				type: 'observation-reconnected',
			},
		],
		expectedNotes: [
			'step 2 rejected capability-not-declared: the resync claims a question request without the questions capability',
			'step 5 rejected session-ended: resync cannot resurrect an ended session',
		],
	},
	{
		id: 'v29-runtime-disposed',
		purpose:
			'After AgentRuntime.dispose() every dispatch is rejected runtime-disposed and neither snapshot nor journal changes.',
		covers: ['runtime-lifecycle', 'rejection'],
		sourceRefs: ['packages/agent/src/AgentRuntime.ts:41-70'],
		assumptions: ['runtime-level behavior, not the pure reducer'],
		events: [obs('sess-main', 1), ev('sess-main', 0, 2, 'session-idle-observed')],
		disposeAfterStep: 1,
		journalReads: [{ afterRevision: 0 }],
	},
];

// ---- execution ----

function expandEvents(vector) {
	const events = [...(vector.events ?? [])];
	if (vector.program !== undefined) {
		const { count, eventIdPrefix, sequenceStart, template } = vector.program;
		for (let index = 0; index < count; index += 1) {
			events.push({
				...template,
				eventId: `${eventIdPrefix}${index + 1}`,
				sequence: sequenceStart + index,
			});
		}
	}
	return events;
}

function summarizeSession(session) {
	return {
		explanation: explainAgentSession(session),
		matchedWaitConditions: ALL_WAIT_CONDITIONS.filter(
			(condition) => matchingAgentWaitConditions(session, [condition]).length > 0,
		),
		snapshot: session,
		status: explainAgentSession(session).status,
		summary: projectAgentSessionSummary(session),
	};
}

function runVector(vector) {
	const runtime = new AgentRuntime();
	const events = expandEvents(vector);
	const steps = [];
	events.forEach((event, eventIndex) => {
		const transition = runtime.dispatch(event);
		steps.push({
			eventIndex,
			kind: transition.kind,
			revision: transition.snapshot.revision,
			semanticEvents: transition.semanticEvents,
			...(transition.kind === 'rejected' ? { reason: transition.reason } : {}),
		});
		if (vector.disposeAfterStep !== undefined && eventIndex === vector.disposeAfterStep - 1) {
			runtime.dispose();
		}
	});
	const snapshot = runtime.getSnapshot();
	const sessions = Object.fromEntries(
		snapshot.sessions.map((session) => [session.sessionId, summarizeSession(session)]),
	);
	const journalReads = (vector.journalReads ?? []).map((spec) => {
		const reader = spec.onEmptyRuntime === true ? new AgentRuntime() : runtime;
		return {
			input: { afterRevision: spec.afterRevision, limit: spec.limit ?? 100 },
			note: spec.note ?? null,
			...(spec.onEmptyRuntime === true ? { onEmptyRuntime: true } : {}),
			result: reader.readChanges({
				afterRevision: spec.afterRevision,
				limit: spec.limit ?? 100,
			}),
		};
	});
	const aggregate =
		vector.aggregateSessionIds === undefined
			? null
			: {
					input: { sessionIds: [...vector.aggregateSessionIds] },
					result: projectAgentSessionGroup(
						snapshot.sessions.filter((session) =>
							vector.aggregateSessionIds?.includes(session.sessionId),
						),
					),
				};
	const location =
		vector.locationPresentationId === undefined
			? null
			: {
					input: { presentationId: vector.locationPresentationId },
					result: projectAgentLocation(snapshot, vector.locationPresentationId),
				};
	return {
		aggregate,
		journalReads,
		location,
		sessions,
		steps,
	};
}

function buildCorpus() {
	return {
		format: 'orchbee.agent-canonical-conformance',
		formatVersion: 1,
		generatedBy: 'packages/agent conformance/generate-corpus.mjs',
		provenance: {
			baseCommit,
			executedImplementation: '@orchbee/agent dist build of the fingerprinted sources',
			note: 'expectations are produced by the authoritative TypeScript reducer/journal/projection, not hand-written; parity evidence, not public schema authority',
			sourceFingerprints: collectSourceFingerprints(),
			zod: require('zod/package.json').version,
		},
		vectors: VECTORS.map((vector) => ({
			...(vector.aggregateSessionIds === undefined
				? {}
				: { aggregateSessionIds: [...vector.aggregateSessionIds] }),
			assumptions: vector.assumptions,
			covers: vector.covers,
			...(vector.disposeAfterStep === undefined
				? {}
				: { disposeAfterStep: vector.disposeAfterStep }),
			events: expandEvents(vector).map((event, index) => ({
				event,
				...(vector.program !== undefined && index >= (vector.events?.length ?? 0)
					? { programGenerated: true }
					: {}),
			})),
			expected: runVector(vector),
			expectedNotes: vector.expectedNotes ?? [],
			id: vector.id,
			...(vector.locationPresentationId === undefined
				? {}
				: { locationPresentationId: vector.locationPresentationId }),
			purpose: vector.purpose,
			sourceRefs: vector.sourceRefs,
		})),
	};
}

// The fixture is committed in repository-normalized (oxfmt) form so that
// `format:check` and `--check` observe identical bytes.
function renderCorpus(corpusValue) {
	const raw = `${JSON.stringify(corpusValue, null, '\t')}\n`;
	const formatted = spawnSync('pnpm', ['exec', 'oxfmt', '--stdin-filepath', OUTPUT], {
		cwd: repoRoot,
		encoding: 'utf8',
		input: raw,
	});
	if (formatted.status !== 0) {
		throw new Error(`oxfmt failed on rendered corpus: ${formatted.stderr || formatted.error}`);
	}
	return formatted.stdout;
}

const corpus = buildCorpus();
const rendered = renderCorpus(corpus);
if (CHECK_MODE) {
	if (!existsSync(OUTPUT)) {
		console.error(`conformance check failed: missing fixture ${OUTPUT}`);
		process.exit(1);
	}
	const current = readFileSync(OUTPUT, 'utf8');
	if (current !== rendered) {
		console.error(
			`conformance check failed: ${OUTPUT} is stale relative to the fingerprinted sources — run pnpm -F @orchbee/agent conformance:generate`,
		);
		process.exit(1);
	}
	console.log('conformance corpus is up to date');
} else {
	mkdirSync(dirname(OUTPUT), { recursive: true });
	writeFileSync(OUTPUT, rendered);
	console.log(`wrote ${OUTPUT}`);
}
