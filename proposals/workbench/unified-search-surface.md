# Unified Search Surface

| Field             | Value                                                                                                                                                                    |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Status            | Accepted; migration phases 1–4 implemented                                                                                                                               |
| Owner             | `@orchbee/workbench` maintainers and application composition owners                                                                                                      |
| Current authority | Future Workbench product design only; [`docs/frontend/actions.md`](../../docs/frontend/actions.md) governs the cross-workspace action contract                           |
| Exit condition    | Implement the accepted behavior, move current public and maintenance facts into Workbench-owned guides, models, and tests, then delete this proposal and its translation |

Simplified Chinese review translation:
[unified-search-surface.zh-CN.md](unified-search-surface.zh-CN.md). English remains canonical.

## Decision

Build one Workbench-owned search surface with provider-backed filters and two global entry presets:

| Entry                                  | Initial filter | Initial query | Purpose                                                         |
| -------------------------------------- | -------------- | ------------- | --------------------------------------------------------------- |
| Visible search control or `Mod+K`      | All            | Empty         | Discover recent entities, common actions, and settings          |
| Visible Actions entry or `Mod+Shift+P` | Actions        | Empty         | Discover and run an action without knowing its menu or binding  |
| Reviewed contextual open request       | Declared       | Declared      | Open one preset without creating a second search implementation |

All product operations selected through the surface invoke the Action Runtime defined by
[`docs/frontend/actions.md`](../../docs/frontend/actions.md). Search query, filter, highlight,
scroll, nested-picker, and provider-loading state remain local to the mounted surface.

## Problem

A person needs one predictable place to discover and invoke Orchbee capabilities without first
learning every menu location or shortcut. Empty-query content must teach the product through useful
groups and explanations; typed-query content must rank results deterministically across independent
providers.

Current Workbench search is registered as `workbench.search.open`; the sidebar and shortcut binding
invoke the same Action, whose handler delegates to an optional host capability. The accepted design
replaces that host capability with the search surface when its model and presentation are
implemented together.

## Scope

| Included                                                                                 | Excluded from the first implementation                                             |
| ---------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| All, Agents, Actions, and Settings filters                                               | File search                                                                        |
| Curated zero-query groups and deterministic typed-query ranking                          | User-authored macros or a shell-like command language                              |
| Local and asynchronous search providers                                                  | AI-generated or opaque ranking                                                     |
| Nested argument pickers for actions that need input                                      | Arbitrary action chaining or result piping                                         |
| Disabled explanations, confirmation handoff, execution presentation, and result feedback | Persistent query history or persistent action MRU                                  |
| Keyboard, pointer, focus, screen-reader, localization, and reduced-motion behavior       | User-editable keybindings                                                          |
| An agent-prefill fallback that never submits automatically                               | Automatic remote or automation exposure of local actions                           |
| Later admission of Workspaces, Automations, and Skills through owned providers           | Committing those filters before stable searchable read models and real usage exist |

## Ownership

| Responsibility                                                 | Owner                                                                                  |
| -------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Search-surface product behavior, state model, and presentation | `@orchbee/workbench`                                                                   |
| Mounted action registry and action execution                   | The product runtime under [`docs/frontend/actions.md`](../../docs/frontend/actions.md) |
| Agent result data and navigation action                        | The agent capability owner                                                             |
| Stable settings destinations and navigation action             | The settings capability owner                                                          |
| Provider composition, platform category, and host adapters     | The composition application                                                            |
| Generic dialog, command-list, and listbox primitives           | `@orchbee/ui` only when free of Orchbee entities, messages, and platform effects       |
| Product descriptors and both shipped translations              | `@orchbee/i18n`                                                                        |
| Storage, process, transport, routing, and remote-job lifecycle | Their existing application or runtime owners                                           |

Do not create a package for providers, search Hooks, Stores, or types. Keep each searchable read
model with its semantic owner and compose providers through narrow public contracts.

## Filters

| Filter   | Content                                                                                 |
| -------- | --------------------------------------------------------------------------------------- |
| All      | A quota-balanced merge of Agents, Actions, and Settings                                 |
| Agents   | Recent and matching agents supplied by the agent owner                                  |
| Actions  | Registered actions visible in the current scope                                         |
| Settings | Stable destinations that invoke the settings navigation action with a bound settings ID |

Files are deliberately absent. Workspaces, Automations, and Skills may appear as owned groups in
All only after they expose stable searchable read models. Promote a group to a persistent filter
only when its volume and repeated use justify the navigation cost.

Switching filters during one open session preserves the query. A global entry always starts from its
declared filter and an empty query. Closing discards query, highlight, scroll, provider errors, and
nested-picker state.

## Zero-query content

| Filter   | Initial groups                                                                                      |
| -------- | --------------------------------------------------------------------------------------------------- |
| All      | Recent Agents, Recommended Actions, Mode, Terminal, View, and selected Settings destinations        |
| Agents   | Recent Agents followed by the agent owner's empty or loading state                                  |
| Actions  | Recent Actions for the application session followed by curated feature categories                   |
| Settings | Recently opened destinations when already available in owned state, then stable settings categories |

- Group order is product-curated and deterministic.
- Feature owners contribute category and recommendation metadata; they do not assign absolute
  positions across unrelated owners.
- Provider and group quotas prevent one source from displacing every other useful All result.
- Disabled actions remain visible when their reason teaches how to make them available.
- Hidden actions never reserve a row or empty group.
- Action recency remains in memory for the first implementation. Persistence requires a separate
  versioned storage contract and privacy review.

## Providers

Each provider returns stable result identity, prepared presentation, and one scope-bound Action
invocation:

```ts
type SearchResultInvocation =
	| {
			readonly kind: 'ready';
			readonly prepared: PreparedActionInvocation;
	  }
	| {
			readonly kind: 'requires-input';
			readonly unbound: UnboundActionInvocation;
	  };

interface SearchResult {
	readonly id: SearchResultId;
	readonly providerId: SearchProviderId;
	readonly group: SearchResultGroup;
	readonly label: string;
	readonly description?: string;
	readonly icon?: ProductIconName;
	readonly invocation: SearchResultInvocation;
	readonly availability: ActionAvailability;
	readonly score: SearchScore;
}

interface SearchProvider {
	readonly id: SearchProviderId;
	getDefaultSections(context: SearchContext): SearchSection[] | Promise<SearchSection[]>;
	search(
		query: SearchQuery,
		context: SearchContext,
		signal: AbortSignal,
	): SearchResult[] | Promise<SearchResult[]>;
}
```

Providers return values; they never mutate the surface Store. The surface tags every request with an
open-session ID and query revision and accepts a completion only when both still match.

The Action provider requests subtree discovery from the application Action scope so Workbench,
Tab Strip, and Panel registrations can participate without a second registry. It preserves the
runtime-provided scope in every prepared or unbound invocation. It never reconstructs an invocation
from a bare action ID or re-resolves a result through whichever sibling happens to be active later.
Both invocation variants are runtime-owned tokens. An argument picker binds input with
`prepareInvocation(...)`; a stale or copied token cannot fall back to another registration.

### Async coordination

- Render local Action and Settings results in the first frame.
- Show asynchronous results progressively without changing the highlighted result ID.
- Give every revision one `AbortController`; abort it when query, filter, context, or session changes.
- Ignore late completion even when a provider fails to observe cancellation.
- Isolate provider failure to its group while other providers remain usable.
- Bound provider results; completion order never changes final ordering.
- Closing the surface cancels search-provider work but does not cancel an accepted product action
  unless that action explicitly assigns execution ownership to the surface.

Provider search is not an Action. It is read-model work owned by the open surface. Selecting a
result invokes the bound Action under the action execution and asynchronous-lifecycle standard.

## Matching and ranking

Rank by these learnable tiers:

1. exact localized title;
2. localized title prefix;
3. localized title token prefix;
4. explicitly authored localized keyword or alias;
5. canonical English title or alias when the active locale differs; and
6. fuzzy title or keyword match.

Within one tier, apply feature recommendation, session recency, provider priority, then stable result
ID. Exact matches always outrank recency. Do not use opaque personalization or send queries to an AI
service in the first implementation.

- Normalize Unicode, case, and repeated whitespace without changing displayed text.
- Search localized titles, approved translated keywords, canonical English fallback, and explicit
  stable aliases.
- Chinese search uses Chinese titles and authored aliases. Generated pinyin indexing remains
  excluded until quality, package size, and terminology behavior are reviewed.
- Never index hidden actions, raw errors, terminal content, agent transcripts, secrets, or unrelated
  user data.
- Provider quotas apply to All; an explicit filter may show the provider's full bounded result set.

## Nested action input

Selecting an action with unresolved required input opens a nested picker in the same surface.

- The picker owns its prompt, query, results, validation, loading, empty, error, and cancellation
  states.
- Back restores the parent query, filter, scroll position, and highlighted result.
- Final selection passes collected input and the original unbound token to
  `prepareInvocation(...)`, then executes the resulting prepared token.
- A picker may use one provider admitted by the action owner; it does not gain unrestricted access
  to every search provider.
- The first implementation has no generic pipeline or action-composition language.

## Agent fallback

When an All or Agents query has no direct match, the agent owner may contribute a final
`Ask Agent about "{query}"` result.

- Selection opens an agent composer with the query prefilled.
- It never sends a message, starts a process, or incurs cost without a separate explicit submit.
- Omit it for an empty query, unsupported context, or sensitive surface.
- `@orchbee/i18n` owns the complete label and accessible name in both locales.

## Interaction model

| State              | Required data                                                                                 |
| ------------------ | --------------------------------------------------------------------------------------------- |
| `closed`           | Semantic focus-return target                                                                  |
| `browsing`         | Session, filter, query revision, query, sections, provider states, scroll, and highlighted ID |
| `picking-argument` | Parent snapshot, action ID, picker step, query revision, sections, scroll, and highlighted ID |
| `confirming`       | Bound Action execution observing feature-owned confirmation                                   |
| `observing-action` | Execution ID and surface presentation for an action whose policy keeps the surface open       |

| Event                               | Result                                                                                              |
| ----------------------------------- | --------------------------------------------------------------------------------------------------- |
| Open                                | Capture semantic focus, create session, apply preset, and focus query input                         |
| Query changed                       | Increment revision, cancel old provider work, retain valid highlight, and request current providers |
| Provider completed                  | Accept only current session and revision, then merge deterministically                              |
| Move selection                      | Select one visible result by stable ID; disabled results remain inspectable                         |
| Invoke enabled result               | Enter argument picker or invoke its Action                                                          |
| Invoke disabled result              | Preserve state and expose the localized reason                                                      |
| Registry or context changed         | Re-project Actions and retain highlight only while the same result remains visible                  |
| Escape in nested state              | Cancel that step and restore its parent snapshot                                                    |
| Escape in browsing or outside click | Close without invocation and restore focus                                                          |
| Surface unmounted                   | Abort provider work, unsubscribe from Action executions, and dispose subscriptions                  |

### Invariants

- A highlighted result resolves in the current session, revision, filter, and visible result set.
- A stale provider completion never enters the observable projection.
- Provider completion order does not change final grouped order.
- A disabled or hidden action never executes.
- One accepted result invocation reaches at most one Action execution.
- Cancelling or closing before invocation commits no Action.
- Focus resolves to the query, an owned nested control, or a valid semantic return target.

## Action execution presentation

Action execution follows [`docs/frontend/actions.md`](../../docs/frontend/actions.md#execution-contract).
The surface only decides how to present that execution:

| Policy            | Surface behavior                                                                                    |
| ----------------- | --------------------------------------------------------------------------------------------------- |
| Default           | Close when execution starts; result appears through the owning action or application result channel |
| `remain-open`     | Observe the same execution ID, prevent duplicate invocation, and show owned progress or result      |
| Background action | Close; an application-owned activity or notification surface continues observation                  |
| External job      | Close or navigate as declared; the external owner remains authoritative through a stable job ID     |

Closing the surface never turns a running action into cancelled by implication. A late completion
does not reopen the surface, restore stale focus, or navigate away from later user work.

## Interface

The desktop presentation is a centered, upper-viewport modal overlay using approved Workbench and
design-system geometry. Input, filters, and footer remain visible while result groups scroll.

```text
┌──────────────────────────────────────────────────────────────┐
│ Search agents, actions, and settings…                        │
├──────────────────────────────────────────────────────────────┤
│ All   Agents   Actions   Settings                            │
├──────────────────────────────────────────────────────────────┤
│ Recent Agents                                                │
│  icon  Agent title                 workspace          time   │
│                                                              │
│ Terminal                                                     │
│  icon  Split panel right                         shortcut    │
│  icon  Close terminal       Disabled reason                  │
│                                                              │
│ Settings                                                     │
│  icon  Appearance                                Settings ›  │
├──────────────────────────────────────────────────────────────┤
│ ↑↓ Select      Enter Open or Run      Esc Back or Close      │
└──────────────────────────────────────────────────────────────┘
```

| Region        | Contract                                                                                                 |
| ------------- | -------------------------------------------------------------------------------------------------------- |
| Input         | Initial focus, persistent while browsing, and no required instruction only in the placeholder            |
| Filters       | Tablist with visible and programmatic selection                                                          |
| Group heading | Quiet semantic category with enough separation from rows                                                 |
| Result row    | Product icon, primary label, optional description or context, and trailing resolved shortcut or metadata |
| Footer        | Only registered interactions valid in the current state; guidance, never the sole instruction source     |
| Scroll region | Result groups only; input, filters, and footer remain visible                                            |

- Use the typography, hierarchy, spacing, radius, control, icon, border, surface, focus, and shadow
  roles from [`docs/frontend/visual-foundations.md`](../../docs/frontend/visual-foundations.md).
- Keep one icon column and one trailing metadata column.
- Prefer one-line titles; required disabled reasons, errors, and instructions never truncate.
- Keyboard highlight wins over hover; focus remains independently visible.
- Avoid animation that moves selectable rows; preserve highlighted visual position when practical.
- A narrow viewport keeps the same semantic regions. A later mobile sheet is a presentation adapter,
  not another search model.

## Empty, loading, and failure states

| Condition                               | Presentation                                                                               |
| --------------------------------------- | ------------------------------------------------------------------------------------------ |
| Pending provider with local results     | Show local groups immediately and loading only in the pending group                        |
| Pending providers without local results | Show a localized loading state instead of a blank panel                                    |
| No match                                | Name the searched filter and offer the reviewed agent-prefill fallback when available      |
| One provider failed                     | Show localized retry in that group and keep other groups usable                            |
| Every provider failed                   | Show one focused error state with retry and close actions                                  |
| Action disabled                         | Preserve discovery and expose its localized reason                                         |
| Remain-open Action running              | Prevent duplicate invocation and show only progress owned by that Action                   |
| Results changed around highlight        | Preserve the same result ID; otherwise select the nearest enabled result deterministically |

## Focus and accessibility

- Use a modal dialog with one accessible name.
- Implement the query as a combobox controlling a listbox; provider headings are associated groups
  and results are options.
- Implement filters as a named tablist. Do not merge tab, option, and button semantics.
- Keep DOM focus in the query and move visual selection with a complete active-descendant pattern.
- Up and Down move results; Enter invokes an enabled result; Escape returns one nested level or
  closes browsing; composition Enter never invokes.
- Tab follows documented focus order and is not an undocumented filter-cycle command.
- Announce result-count changes, provider failures, disabled reasons, nested steps, and observed
  action results without repeating the list.
- Outside-click dismissal never clicks through.
- Restore a still-valid semantic focus target or the owning surface's fallback.
- Reduced motion removes nonessential entry, exit, and row-reposition animation.
- Defer a filter-switching shortcut until conflict analysis proves it does not override navigation
  or browser behavior; the footer never advertises an unregistered chord.

## Localization and privacy

- Every title, description, category, keyword, placeholder, group, empty state, error, disabled
  reason, confirmation, accessible name, progress label, and footer instruction ships through
  `@orchbee/i18n` in `en-US` and `zh-CN`.
- Stable IDs, user values, paths, commands, terminal output, and external agent content are not
  translated.
- Platform formatters own shortcut glyphs and labels.
- Keep query text in mounted memory for the first implementation.
- Do not emit query text, result labels, agent names, terminal content, bound user input, or
  confirmation copy to telemetry.
- Search results never execute on highlight, hover, preview, or prefetch.

## Performance

- Open with focused input and local Action and Settings results within `100 ms` on a measured,
  representative supported desktop environment.
- Typing updates the input and local results without waiting for asynchronous providers.
- Keep matching, ranking, grouping, and state transitions pure, bounded, and React-independent.
- Build the static Action index once per locale or registry revision, not per row render.
- Virtualize only after measurement shows bounded results cannot meet the interaction target.
- Record dataset, environment, and percentile for performance evidence.

## Verification

| Unit or boundary         | Required evidence                                                                                        |
| ------------------------ | -------------------------------------------------------------------------------------------------------- |
| Search reducer           | Every transition, stale completion, selection stability, nested restoration, close, and invariants       |
| Ranking                  | Locale terms, ranking tiers, deterministic ties, quotas, and provider-order independence                 |
| Provider coordinator     | Cancellation, late completion, partial failure, retry, registry change, and bounded results              |
| Action integration       | Visible result reaches the same operation as Button, Menu, and Shortcut paths                            |
| Focus and accessibility  | Open, query, filters, nested picker, close, late provider and Action completion, zoom, and screen reader |
| Localization and privacy | Both locales, wrapping, disabled reason, long metadata, redaction, and no automatic Agent submission     |

Async provider ordering and cancellation require a TLA+ model before production integration. Model
session, query revision, provider request and completion, cancellation, visible revision, selection,
close, and reopen.

| Claim                                                           | Evidence                                                          |
| --------------------------------------------------------------- | ----------------------------------------------------------------- |
| Stale completion never enters a newer query or reopened session | TLA+ safety property and deterministic reordered-completion tests |
| Selection resolves in the current visible result set            | Reducer invariant and stateful property tests                     |
| Final ranking is independent of provider completion order       | Pure ranking property test                                        |
| Closing or nested cancellation commits no Action invocation     | Reducer tests and deterministic browser traces                    |
| Late provider or Action completion does not steal focus         | Deterministic browser traces with semantic focus checkpoints      |

TLA+ progress may assume an uncancelled current provider eventually completes. It does not claim
network availability, provider correctness, or action-domain correctness.

Storybook covers both entry modes, four filters, empty and typed queries, local and async states,
partial and total failure, enabled and disabled actions, nested input, confirmation handoff,
remain-open execution, both locales, narrow layout, zoom, high contrast, dark theme, and reduced
motion.

## Migration

Phases 1–4 are implemented. Phases 5–8 remain future work governed by this proposal.

| Phase | Change                                                                                                                       | Completion evidence                                                                |
| ----- | ---------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| 1     | Implement and test the Action Runtime required by the frontend action standard                                               | Runtime, execution, async lifecycle, concurrency, localization, and scope evidence |
| 2     | Register current Workbench actions and adapt existing callbacks through one domain commit path                               | Stable IDs, descriptors, availability, and compatibility tests                     |
| 3     | Route visible controls and menus through the runtime                                                                         | Existing interaction tests plus same-operation contracts                           |
| 4     | Replace independent Workbench keydown branches with the enumerable shortcut resolver                                         | Shortcut matrix, conflict tests, and old-listener removal                          |
| 5     | Implement the search state model, surface, local Actions and Settings providers, accessibility, and Stories                  | Deterministic core, TLA+, component, Storybook, and browser evidence               |
| 6     | Add the Agents provider and no-auto-submit fallback through agent-owned public contracts                                     | Cancellation, ranking, privacy, empty-state, and fallback tests                    |
| 7     | Adapt active Terminal instance actions at the application composition boundary                                               | Instance disposal, active scope, localized presentation, and terminal tests        |
| 8     | Remove duplicate callbacks and metadata; update Workbench current guides, APIs, project contract, and tests; delete proposal | No competing path, package checks, and root `pnpm verify`                          |

The package-local `TerminalActionRegistry` may retain per-instance terminal semantics. The
composition adapter maps compatible definitions and outcomes without making `@orchbee/terminal`
depend on Workbench presentation or localization.

## Consequences

- New users gain categorized discovery, descriptions, disabled reasons, and shortcut hints.
- Controls, menus, shortcuts, and results share one Action execution path.
- Async search has one cancellation and stale-result model.
- Feature owners must expose stable read models, Action invocations, and localized descriptors.
- Provider order, focus, accessibility, and async completion require maintained deterministic and
  formal evidence.
- Product-specific behavior remains here until implementation; accepted cross-workspace Action rules
  remain only in [`docs/frontend/actions.md`](../../docs/frontend/actions.md).
