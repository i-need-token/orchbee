// The record of existing Rust module-graph findings, and the rule that the graph may not grow
// beyond it: a cycle edge or an upward reference that the record does not cover fails the check.
// The functions here take analysis results and record text; the command owns the file itself.

export const RECORD_PATH = 'scripts/rust-module-graph-record.txt';
const SUMMARY_ENTRY_LIMIT = 8;
const LOCATION_LIMIT = 6;

const HEADER = [
	'# Existing Rust module cycle edges and upward references, one per line.',
	'# `pnpm lint:rust-modules` fails when the module graph grows beyond this record.',
	'# Never add or change a line by hand; the rule below says when an entry may be recorded.',
	'# After removing a cycle or an upward reference, run `pnpm lint:rust-modules -- --tighten`',
	'# and commit the smaller file. docs/LINTING.md#rust-module-record defines the rule.',
];
const CYCLE_LINE = /^cycle (\S+) (lib|bin) (\S+) -> (\S+)$/u;
const UPWARD_LINE = /^upward (\S+) (lib|bin) (\S+) -> (\S+) in (\S+) ([1-9]\d*)$/u;

function targetKey(result) {
	return `${result.member} ${result.target}`;
}

function targetLabel(key) {
	const [member, target] = key.split(' ');
	return `${member} (${target})`;
}

function pairKey(entry) {
	return `${entry.from} -> ${entry.to}`;
}

function compareText(left, right) {
	if (left === right) return 0;
	return left < right ? -1 : 1;
}

/**
 * Lists what the record counts in one analyzed target: sibling edges inside a module cycle, except
 * edges that a layer contract allows because they point to a lower layer, and upward references
 * grouped by assigned module pair.
 */
function countedFindings(result) {
	const edges = new Map();
	for (const cycle of result.cycles) {
		const prefix = cycle.parent === '' ? '' : `${cycle.parent}::`;
		for (const link of cycle.links) {
			if (link.downward === true) continue;
			const edge = { from: `${prefix}${link.from}`, to: `${prefix}${link.to}`, cycle, link };
			edges.set(pairKey(edge), edge);
		}
	}
	const pairs = new Map();
	for (const violation of result.layers?.violations ?? []) pairs.set(pairKey(violation), violation);
	return { edges, pairs };
}

function recordLines(results) {
	const lines = [];
	for (const result of results) {
		const key = targetKey(result);
		const { edges, pairs } = countedFindings(result);
		for (const edge of edges.keys()) lines.push(`cycle ${key} ${edge}`);
		for (const [pair, violation] of pairs) {
			for (const source of violation.sources)
				lines.push(`upward ${key} ${pair} in ${source.module} ${source.references}`);
		}
	}
	return lines.toSorted(compareText);
}

/** Renders the record that states exactly the analyzed findings. */
export function formatRecord(results) {
	return `${[...HEADER, ...recordLines(results)].join('\n')}\n`;
}

/** Reads record text into per-target cycle edges and upward pairs; malformed lines are problems. */
export function parseRecord(text, label = RECORD_PATH) {
	const targets = new Map();
	const problems = [];
	const seen = new Set();
	const entry = (member, target) => {
		const key = `${member} ${target}`;
		const found = targets.get(key) ?? { edges: new Map(), pairs: new Map() };
		targets.set(key, found);
		return found;
	};
	for (const [index, raw] of text.replaceAll('\r\n', '\n').split('\n').entries()) {
		const line = raw.trimEnd();
		if (line === '' || line.startsWith('#')) continue;
		const cycle = CYCLE_LINE.exec(line);
		const upward = cycle === null ? UPWARD_LINE.exec(line) : null;
		const identity = upward === null ? line : line.slice(0, line.lastIndexOf(' '));
		if (cycle === null && upward === null) {
			problems.push(`${label}:${index + 1}: not a record line: ${line}`);
		} else if (seen.has(identity)) {
			problems.push(`${label}:${index + 1}: duplicate record line: ${line}`);
		} else if (cycle !== null) {
			const edge = { from: cycle[3], to: cycle[4] };
			entry(cycle[1], cycle[2]).edges.set(pairKey(edge), edge);
		} else {
			const { pairs } = entry(upward[1], upward[2]);
			const pair = { from: upward[3], to: upward[4], references: 0, sources: new Map() };
			const found = pairs.get(pairKey(pair)) ?? pair;
			pairs.set(pairKey(pair), found);
			found.sources.set(upward[5], Number(upward[6]));
			found.references += Number(upward[6]);
		}
		seen.add(identity);
	}
	return { targets, problems };
}

function reaches(links, start, goal, removed) {
	const previous = new Map([[start, undefined]]);
	const queue = [start];
	for (let index = 0; index < queue.length; index += 1) {
		const node = queue[index];
		if (node === goal) {
			const route = [];
			for (let step = node; step !== undefined; step = previous.get(step)) route.unshift(step);
			return route;
		}
		for (const link of links) {
			if (link === removed || link.from !== node || previous.has(link.to)) continue;
			previous.set(link.to, node);
			queue.push(link.to);
		}
	}
	return undefined;
}

/**
 * Describes each uncovered edge: the shortest loop it closes, and how many of the other uncovered
 * edges of its cycle group would leave every cycle if that one edge were taken out. One new
 * reference can pull many older edges into a cycle; this count points at it.
 */
function describeNewEdges(edges) {
	return edges.map((edge) => {
		const { links } = edge.cycle;
		const others = edges.filter((other) => other !== edge && other.cycle === edge.cycle);
		return {
			from: edge.from,
			to: edge.to,
			location: edge.link.sample,
			loop: [edge.link.from, ...(reaches(links, edge.link.to, edge.link.from) ?? [])],
			scope: edge.cycle.parent,
			others: others.length,
			clears: others.filter(
				(other) => reaches(links, other.link.to, other.link.from, edge.link) === undefined,
			).length,
		};
	});
}

/**
 * Compares analyzed targets with the record. A target fails when it holds more uncovered cycle
 * edges, or more upward references in a module pair, than recorded entries of modules that no
 * longer exist can account for; those entries stand for a renamed or moved module. Recorded
 * entries that are merely gone never fail and never make room for a new one. `complete` says the
 * results cover every Cargo member, so a recorded target without a result is gone too.
 */
export function compareWithRecord(results, record, { complete = true } = {}) {
	const current = new Set(recordLines(results));
	const analyzed = new Set(results.map(targetKey));
	let outdated = 0;
	let edgeCount = 0;
	let referenceCount = 0;
	for (const [key, recorded] of record.targets) {
		if (!analyzed.has(key) && !complete) continue;
		for (const edge of recorded.edges.keys()) {
			edgeCount += 1;
			if (!current.has(`cycle ${key} ${edge}`)) outdated += 1;
		}
		for (const [pair, { sources }] of recorded.pairs) {
			for (const [module, references] of sources) {
				referenceCount += references;
				if (!current.has(`upward ${key} ${pair} in ${module} ${references}`)) outdated += 1;
			}
		}
	}
	const targets = [];
	for (const result of results) {
		const recorded = record.targets.get(targetKey(result)) ?? {
			edges: new Map(),
			pairs: new Map(),
		};
		const { edges, pairs } = countedFindings(result);
		const exists = new Set(result.modulePaths);
		const uncoveredEdges = [...edges].filter(([key]) => !recorded.edges.has(key));
		const replacedEdges = [...recorded.edges].filter(
			([key, edge]) => !edges.has(key) && !(exists.has(edge.from) && exists.has(edge.to)),
		).length;
		const grownPairs = [];
		let grownReferences = 0;
		for (const [key, violation] of pairs) {
			const allowed = recorded.pairs.get(key);
			if (violation.references <= (allowed?.references ?? 0)) continue;
			grownReferences += violation.references - (allowed?.references ?? 0);
			grownPairs.push({
				from: violation.from,
				fromLayer: violation.fromLayer,
				to: violation.to,
				toLayer: violation.toLayer,
				references: violation.references,
				recorded: allowed?.references ?? 0,
				sources: violation.sources
					.map((source) => ({ ...source, recorded: allowed?.sources.get(source.module) ?? 0 }))
					.filter((source) => source.references > source.recorded),
			});
		}
		let replacedReferences = 0;
		for (const [key, pair] of recorded.pairs) {
			if (!pairs.has(key) && !(exists.has(pair.from) && exists.has(pair.to)))
				replacedReferences += pair.references;
		}
		const newEdges = Math.max(0, uncoveredEdges.length - replacedEdges);
		const newReferences = Math.max(0, grownReferences - replacedReferences);
		if (newEdges === 0 && newReferences === 0) continue;
		targets.push({
			label: targetLabel(targetKey(result)),
			newEdges,
			replacedEdges,
			edges: newEdges === 0 ? [] : describeNewEdges(uncoveredEdges.map(([, edge]) => edge)),
			newReferences,
			replacedReferences,
			pairs: newReferences === 0 ? [] : grownPairs,
		});
	}
	return { failed: targets.length > 0, targets, outdated, edgeCount, referenceCount };
}

function plural(count, noun) {
	return `${count.toLocaleString('en-US')} ${noun}${count === 1 ? '' : 's'}`;
}

function bounded(entries, details, lines, render) {
	const shown = details ? entries : entries.slice(0, SUMMARY_ENTRY_LIMIT);
	for (const entry of shown) render(entry);
	if (shown.length < entries.length)
		lines.push(`    ... ${entries.length - shown.length} more; use --details`);
}

function formatFailure(verdict, details, recordPath) {
	const lines = [`error: the Rust module graph grew beyond ${recordPath}`];
	for (const target of verdict.targets) {
		if (target.newEdges > 0) {
			lines.push(`${target.label}: ${plural(target.newEdges, 'new cycle edge')}`);
			if (target.replacedEdges > 0)
				lines.push(
					`  ${plural(target.edges.length, 'edge')} below are not recorded; ${target.replacedEdges} only replace recorded edges of modules that no longer exist, so ${target.newEdges} are new.`,
				);
			const edges = target.edges.toSorted(
				(left, right) => right.clears - left.clears || compareText(pairKey(left), pairKey(right)),
			);
			bounded(edges, details, lines, (edge) => {
				const scope = edge.scope === '' ? '' : ` (modules of ${edge.scope})`;
				lines.push(`  ${edge.from} -> ${edge.to}, first at ${edge.location}`);
				lines.push(`    closes the loop ${edge.loop.join(' -> ')}${scope}`);
				if (edge.clears > 0)
					lines.push(
						`    without this edge, ${edge.clears} of the ${edge.others} other unrecorded edges of its group leave the cycle`,
					);
			});
		}
		if (target.newReferences > 0) {
			lines.push(`${target.label}: ${plural(target.newReferences, 'new upward reference')}`);
			bounded(target.pairs, details, lines, (pair) => {
				lines.push(
					`  ${pair.from} (${pair.fromLayer}) -> ${pair.to} (${pair.toLayer}): ${plural(pair.references, 'reference')}, ${pair.recorded} recorded`,
				);
				for (const source of pair.sources) {
					const locations = details ? source.locations : source.locations.slice(0, LOCATION_LIMIT);
					const more = source.locations.length - locations.length;
					lines.push(
						`    in ${source.module}: ${source.references}, ${source.recorded} recorded: ${locations.join(', ')}${more > 0 ? `, ... ${more} more` : ''}`,
					);
				}
			});
		}
	}
	lines.push(
		'What to do: take the new reference out, or make the dependency point one way: move the',
		'shared item into a module that both sides may depend on, or let the caller pass it in.',
		`Never add or change a line in ${recordPath} by hand.`,
		'If avoiding the new reference would materially expand your task, record it with',
		'`pnpm lint:rust-modules -- --owner-approved`, state the reason in the commit body, and list',
		'the entry in your final report as a decision the owner may veto.',
	);
	return lines;
}

/** Renders the comparison: the failure with its remedy, or the pass line and a tighten notice. */
export function formatVerdict(verdict, { details = false, recordPath = RECORD_PATH } = {}) {
	if (verdict.failed) return `${formatFailure(verdict, details, recordPath).join('\n')}\n`;
	const lines = [
		`No new cycle edge or upward reference; ${recordPath} holds ${plural(verdict.edgeCount, 'existing cycle edge')} and ${plural(verdict.referenceCount, 'upward reference')}.`,
	];
	if (verdict.outdated > 0)
		lines.push(
			`${plural(verdict.outdated, 'recorded line')} no longer match${verdict.outdated === 1 ? 'es' : ''} the tree. Run \`pnpm lint:rust-modules -- --tighten\` and commit the record with your change.`,
		);
	return `${lines.join('\n')}\n`;
}
