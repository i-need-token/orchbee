import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { after, describe, it } from 'node:test';

import {
	RECORD_PATH,
	compareWithRecord,
	formatRecord,
	formatVerdict,
	parseRecord,
} from './rust-module-graph-record.mjs';
import {
	analyzeWorkspace,
	buildModuleGraph,
	checkRecord,
	formatReport,
	parseLayerContract,
} from './rust-module-graph.mjs';

const roots = [];

after(() => {
	for (const root of roots) rmSync(root, { recursive: true, force: true });
});

function write(root, files) {
	for (const [relative, content] of Object.entries(files)) {
		const file = path.join(root, relative);
		mkdirSync(path.dirname(file), { recursive: true });
		writeFileSync(file, content);
	}
}

function fixture(files) {
	const root = mkdtempSync(path.join(tmpdir(), 'orchbee-rust-module-graph-'));
	roots.push(root);
	write(root, files);
	return root;
}

function edges(root, crateRoot) {
	const graph = buildModuleGraph(root, path.join(root, crateRoot));
	const pairs = graph.references.map(
		(reference) => `${reference.from.path.join('::')} -> ${reference.to.path.join('::')}`,
	);
	return { graph, edges: [...new Set(pairs)].toSorted() };
}

const MANIFEST = '[workspace]\nmembers = ["crates/*"]\n';

describe('production module graph', () => {
	it('excludes test-only items, comments, and literals from references', () => {
		const root = fixture({
			'src/lib.rs': [
				'mod alpha;',
				'mod beta;',
				'#[cfg(test)]',
				'mod tests;',
				'#[cfg(any(test, feature = "test-support"))]',
				'pub mod test_support;',
				'',
			].join('\n'),
			'src/alpha.rs': [
				'//! Mentions crate::beta::Gamma only in documentation.',
				'const TEXT: &str = "crate::beta::Gamma";',
				'const RAW: &str = r#"crate::beta::Gamma"#;',
				'/* outer /* crate::beta::Gamma */ still a comment */',
				'#[cfg(test)]',
				'fn helper() -> crate::beta::Gamma { crate::beta::Gamma }',
				'#[cfg(all(windows, test))]',
				'use crate::beta::Gamma;',
				'pub(crate) fn value() -> u8 { 1 }',
				'',
			].join('\n'),
			'src/beta.rs': [
				'pub(crate) struct Gamma;',
				'#[cfg(windows)]',
				'pub(crate) fn platform() -> u8 { crate::alpha::value() }',
				'',
			].join('\n'),
		});
		const { graph, edges: pairs } = edges(root, 'src/lib.rs');
		assert.deepEqual(pairs, ['beta -> alpha']);
		assert.deepEqual(graph.unresolved, []);
		assert.deepEqual([...graph.root.children.keys()].toSorted(), ['alpha', 'beta']);
	});

	it('follows platform path variants, use trees, and re-exports', () => {
		const root = fixture({
			'src/lib.rs': [
				'mod other;',
				'mod parent;',
				'pub mod facade {',
				'    pub use crate::parent::Leaf;',
				'}',
				'',
			].join('\n'),
			'src/parent.rs': [
				'#[cfg_attr(windows, path = "parent/child_windows.rs")]',
				'mod child;',
				'pub(crate) use child::Leaf;',
				'',
			].join('\n'),
			'src/parent/child.rs': 'pub(crate) struct Leaf;\n',
			'src/parent/child_windows.rs': [
				'use crate::other::{self, Item as Renamed};',
				'pub(crate) struct Leaf(Renamed);',
				'',
			].join('\n'),
			'src/other.rs': [
				'use crate::parent::Leaf;',
				'pub(crate) struct Item;',
				'pub(crate) fn make(leaf: Leaf) -> Leaf { leaf }',
				'',
			].join('\n'),
		});
		const { graph, edges: pairs } = edges(root, 'src/lib.rs');
		assert.deepEqual(pairs, [
			'facade -> parent::child',
			'other -> parent::child',
			'parent -> parent::child',
			'parent::child -> other',
		]);
		assert.deepEqual(graph.unresolved, []);
	});
});

describe('workspace report', () => {
	it('reports cycles, upward references, and unassigned modules without failing', () => {
		const root = fixture({
			'Cargo.toml': MANIFEST,
			'crates/demo/Cargo.toml': '[package]\nname = "demo"\n',
			'crates/demo/src/lib.rs': 'mod high;\nmod loose;\nmod low;\n',
			'crates/demo/src/high.rs': [
				'//! The higher module.',
				'',
				'// module-layer: top',
				'',
				'pub(crate) fn top() { super::low::base(); }',
				'',
			].join('\n'),
			'crates/demo/src/low.rs': [
				'// module-layer: base',
				'pub(crate) fn base() {}',
				'pub(crate) fn climb() { crate::high::top(); }',
				'',
			].join('\n'),
			'crates/demo/src/loose.rs': 'mod first;\nmod second;\n',
			'crates/demo/src/loose/first.rs': 'pub(crate) fn a() { super::second::b(); }\n',
			'crates/demo/src/loose/second.rs': 'pub(crate) fn b() { super::first::a(); }\n',
			'crates/demo/docs/module-layers.md': [
				'# Demo',
				'',
				'## Layers',
				'',
				'| Layer | Owns |',
				'| --- | --- |',
				'| `base` | Lower modules |',
				'| `top` | Higher modules |',
				'',
			].join('\n'),
		});
		const report = analyzeWorkspace(root);
		assert.deepEqual(report.problems, []);
		const [result] = report.results;
		assert.deepEqual(
			result.cycles.map((cycle) => [cycle.parent, cycle.members]),
			[
				['', ['high', 'low']],
				['loose', ['first', 'second']],
			],
		);
		assert.deepEqual(
			result.layers.violations.map((entry) => [entry.from, entry.to, entry.references]),
			[['low', 'high', 1]],
		);
		assert.deepEqual(result.layers.unassigned, ['loose']);
		const text = formatReport(report);
		assert.match(text, /^ {2}warning: 2 module cycle\(s\)/mu);
		assert.doesNotMatch(text, /^error:/mu);
	});

	it('rejects malformed layer lines and contracts and reuses a linked layer table', () => {
		const layers = '## Layers\n\n| Layer |\n| --- |\n| `base` |\n| `top` |\n';
		const root = fixture({
			'Cargo.toml': MANIFEST,
			'crates/demo/Cargo.toml': '[package]\nname = "demo"\n',
			'crates/demo/src/lib.rs': [
				'mod a;',
				'mod b;',
				'mod c;',
				'mod inline {',
				'    // module-layer: top',
				'    pub(crate) fn top() {}',
				'}',
				'',
			].join('\n'),
			'crates/demo/src/a.rs': '//! Docs.\n\n// module-layer: middle\n',
			'crates/demo/src/b.rs': '// module-layer: top\n// module-layer: base\n',
			'crates/demo/src/c.rs': 'pub(crate) fn c() {}\n// module-layer: base\n',
			'crates/demo/docs/module-layers.md': layers,
		});
		const malformed = analyzeWorkspace(root);
		assert.deepEqual(malformed.problems, [
			'crates/demo/src/b.rs:2 second module-layer line of one module',
			'crates/demo/src/c.rs:2 module-layer line outside the comment block that opens a module',
			'crates/demo/src/a.rs:3 `a` uses unknown layer `middle`',
		]);
		// `b` and the inline module took a layer; the two refused lines assigned nothing.
		assert.deepEqual(malformed.results[0].layers.unassigned, ['a', 'c']);
		const linked = parseLayerContract(
			'## Layers\n\nSee [shared layers](shared.md#layers).\n',
			'linked.md',
			(target) => (target === 'shared.md' ? { markdown: layers, label: 'shared.md' } : undefined),
		);
		assert.deepEqual(linked.problems, []);
		assert.deepEqual([...linked.layers.keys()], ['base', 'top']);
		const missing = parseLayerContract('## Layers\n\nSee [nothing](absent.md).\n', 'missing.md');
		assert.deepEqual(missing.problems, [
			'missing.md: "## Layers" has neither a layer table nor a link to a contract with one',
		]);
	});
});

// One crate without a layer contract (`plain`: `a` and `b` in a cycle, `c` and `d` apart) and one
// with a contract (`layered`: `low` reaches up into `high`, which may reach down into `low`).
function recordedWorkspace() {
	return fixture({
		'Cargo.toml': MANIFEST,
		'crates/plain/Cargo.toml': '[package]\nname = "plain"\n',
		'crates/plain/src/lib.rs': 'mod a;\nmod b;\nmod c;\nmod d;\n',
		'crates/plain/src/a.rs': 'pub(crate) fn a() { crate::b::b(); }\n',
		'crates/plain/src/b.rs': 'pub(crate) fn b() { crate::a::a(); }\n',
		'crates/plain/src/c.rs': 'pub(crate) fn c() {}\n',
		'crates/plain/src/d.rs': 'pub(crate) fn d() {}\n',
		'crates/layered/Cargo.toml': '[package]\nname = "layered"\n',
		'crates/layered/src/lib.rs': 'mod high;\nmod low;\n',
		'crates/layered/src/high.rs':
			'// module-layer: top\npub(crate) fn top() { crate::low::base(); }\n',
		'crates/layered/src/low.rs':
			'// module-layer: base\nmod first;\nmod second;\npub(crate) fn base() {}\n',
		'crates/layered/src/low/first.rs': 'pub(crate) fn climb() { crate::high::top(); }\n',
		'crates/layered/src/low/second.rs': 'pub(crate) fn rest() {}\n',
		'crates/layered/docs/module-layers.md': [
			'## Layers',
			'',
			'| Layer |',
			'| --- |',
			'| `base` |',
			'| `top` |',
			'',
		].join('\n'),
	});
}

function analyzed(root) {
	const report = analyzeWorkspace(root);
	assert.deepEqual(report.problems, []);
	return report.results;
}

function judge(root, recordText) {
	return compareWithRecord(analyzed(root), parseRecord(recordText));
}

function newEdges(verdict) {
	return verdict.targets.flatMap((target) =>
		target.edges.map((edge) => `${edge.from} -> ${edge.to}`),
	);
}

describe('record of existing findings', () => {
	it('records cycle edges and upward references, and the recorded tree passes', () => {
		const root = recordedWorkspace();
		const text = formatRecord(analyzed(root));
		// `high -> low` points down the layer contract, so only the upward edge counts.
		assert.deepEqual(
			text.split('\n').filter((line) => line !== '' && !line.startsWith('#')),
			[
				'cycle crates/layered lib low -> high',
				'cycle crates/plain lib a -> b',
				'cycle crates/plain lib b -> a',
				'upward crates/layered lib low -> high in low::first 1',
			],
		);
		const verdict = judge(root, text);
		assert.equal(verdict.failed, false);
		assert.equal(verdict.outdated, 0);
		assert.deepEqual(parseRecord('cycle crates/plain lib a -> b\nupward nonsense\n').problems, [
			`${RECORD_PATH}:2: not a record line: upward nonsense`,
		]);
	});

	it('fails a new cycle, a module joining a cycle, and a new upward reference', () => {
		const root = recordedWorkspace();
		const text = formatRecord(analyzed(root));
		write(root, {
			'crates/plain/src/c.rs': 'pub(crate) fn c() { crate::d::d(); }\n',
			'crates/plain/src/d.rs': 'pub(crate) fn d() { crate::c::c(); }\n',
		});
		const fresh = judge(root, text);
		assert.equal(fresh.failed, true);
		assert.deepEqual(newEdges(fresh).toSorted(), ['c -> d', 'd -> c']);

		write(root, {
			'crates/plain/src/c.rs': 'pub(crate) fn c() { crate::a::a(); }\n',
			'crates/plain/src/d.rs': 'pub(crate) fn d() {}\n',
			'crates/plain/src/b.rs': 'pub(crate) fn b() { crate::a::a(); crate::c::c(); }\n',
		});
		const joined = judge(root, text);
		assert.deepEqual(newEdges(joined).toSorted(), ['b -> c', 'c -> a']);
		const message = formatVerdict(joined);
		assert.match(message, /^error: /u);
		assert.ok(message.includes('crates/plain/src/c.rs:1'));
		assert.ok(message.includes(RECORD_PATH));

		write(root, {
			'crates/plain/src/c.rs': 'pub(crate) fn c() {}\n',
			'crates/plain/src/b.rs': 'pub(crate) fn b() { crate::a::a(); }\n',
			'crates/layered/src/low/second.rs': 'pub(crate) fn rest() { crate::high::top(); }\n',
		});
		const upward = judge(root, text);
		assert.equal(upward.failed, true);
		assert.deepEqual(newEdges(upward), []);
		const [pair] = upward.targets.flatMap((target) => target.pairs);
		assert.deepEqual([pair.from, pair.to, pair.references, pair.recorded], ['low', 'high', 2, 1]);
		assert.deepEqual(
			pair.sources.map((source) => [source.module, source.locations]),
			[['low::second', ['crates/layered/src/low/second.rs:1']]],
		);
	});

	it('passes removed findings without making room for different ones', () => {
		const root = recordedWorkspace();
		const text = formatRecord(analyzed(root));
		write(root, { 'crates/plain/src/b.rs': 'pub(crate) fn b() {}\n' });
		const removed = judge(root, text);
		assert.equal(removed.failed, false);
		assert.equal(removed.outdated, 2);

		// The surviving modules `a` and `b` left the record's two entries unused; a cycle elsewhere
		// is still new.
		write(root, {
			'crates/plain/src/c.rs': 'pub(crate) fn c() { crate::d::d(); }\n',
			'crates/plain/src/d.rs': 'pub(crate) fn d() { crate::c::c(); }\n',
		});
		assert.deepEqual(newEdges(judge(root, text)).toSorted(), ['c -> d', 'd -> c']);
	});

	it('passes a renamed module and moved upward references against the unchanged record', () => {
		const root = recordedWorkspace();
		const text = formatRecord(analyzed(root));
		rmSync(path.join(root, 'crates/plain/src/b.rs'));
		write(root, {
			'crates/plain/src/lib.rs': 'mod a;\nmod bee;\nmod c;\nmod d;\n',
			'crates/plain/src/a.rs': 'pub(crate) fn a() { crate::bee::b(); }\n',
			'crates/plain/src/bee.rs': 'pub(crate) fn b() { crate::a::a(); }\n',
			'crates/layered/src/low/first.rs': 'pub(crate) fn climb() {}\n',
			'crates/layered/src/low/second.rs': 'pub(crate) fn rest() { crate::high::top(); }\n',
		});
		const renamed = judge(root, text);
		assert.equal(renamed.failed, false);
		assert.equal(renamed.outdated, 3);

		// A rename covers only as many edges as the renamed module had.
		write(root, {
			'crates/plain/src/bee.rs': 'pub(crate) fn b() { crate::a::a(); crate::c::c(); }\n',
			'crates/plain/src/c.rs': 'pub(crate) fn c() { crate::bee::b(); }\n',
		});
		const grown = judge(root, text);
		assert.equal(grown.failed, true);
		assert.deepEqual(
			grown.targets.map((target) => [target.label, target.newEdges, target.replacedEdges]),
			[['crates/plain (lib)', 2, 2]],
		);
	});

	it('tightens the record only from a passing tree; only the owner path adds an entry', () => {
		const root = recordedWorkspace();
		const file = path.join(root, RECORD_PATH);
		const refused = checkRecord(root, analyzed(root), { rewrite: 'tighten' });
		assert.equal(refused.verdict.failed, true);
		assert.equal(existsSync(file), false);

		mkdirSync(path.dirname(file));
		assert.equal(checkRecord(root, analyzed(root), { rewrite: 'owner-approved' }).rewritten, true);
		const before = readFileSync(file, 'utf8');
		assert.equal(before, formatRecord(analyzed(root)));
		write(root, {
			'crates/plain/src/c.rs': 'pub(crate) fn c() { crate::d::d(); }\n',
			'crates/plain/src/d.rs': 'pub(crate) fn d() { crate::c::c(); }\n',
		});
		assert.equal(checkRecord(root, analyzed(root), { rewrite: 'tighten' }).verdict.failed, true);
		assert.equal(readFileSync(file, 'utf8'), before);

		write(root, {
			'crates/plain/src/b.rs': 'pub(crate) fn b() {}\n',
			'crates/plain/src/d.rs': 'pub(crate) fn d() {}\n',
		});
		assert.equal(checkRecord(root, analyzed(root)).verdict.outdated, 2);
		assert.equal(readFileSync(file, 'utf8'), before);
		const tightened = checkRecord(root, analyzed(root), { rewrite: 'tighten' });
		assert.equal(tightened.rewritten, true);
		assert.equal(tightened.verdict.outdated, 0);
		assert.doesNotMatch(readFileSync(file, 'utf8'), /crates\/plain/u);

		// The removed cycle is no longer recorded, so bringing it back is new.
		write(root, { 'crates/plain/src/b.rs': 'pub(crate) fn b() { crate::a::a(); }\n' });
		assert.equal(checkRecord(root, analyzed(root)).verdict.failed, true);
	});
});
