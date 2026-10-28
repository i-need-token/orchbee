import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { after, describe, it } from 'node:test';

import {
	analyzeWorkspace,
	buildModuleGraph,
	formatReport,
	parseLayerContract,
} from './rust-module-graph.mjs';

const roots = [];

after(() => {
	for (const root of roots) rmSync(root, { recursive: true, force: true });
});

function fixture(files) {
	const root = mkdtempSync(path.join(tmpdir(), 'orchbee-rust-module-graph-'));
	roots.push(root);
	for (const [relative, content] of Object.entries(files)) {
		const file = path.join(root, relative);
		mkdirSync(path.dirname(file), { recursive: true });
		writeFileSync(file, content);
	}
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
	it('reports cycles, upward references, unassigned modules, and stale rows without failing', () => {
		const root = fixture({
			'Cargo.toml': MANIFEST,
			'crates/demo/Cargo.toml': '[package]\nname = "demo"\n',
			'crates/demo/src/lib.rs': 'mod high;\nmod loose;\nmod low;\n',
			'crates/demo/src/high.rs': 'pub(crate) fn top() { super::low::base(); }\n',
			'crates/demo/src/low.rs': [
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
				'## Module assignment',
				'',
				'| Module | Layer |',
				'| --- | --- |',
				'| `gone` | `base` |',
				'| `high` | `top` |',
				'| `low` | `base` |',
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
		assert.deepEqual(result.layers.stale, ['gone']);
		const text = formatReport(report);
		assert.match(text, /^ {2}warning: 2 module cycle\(s\)/mu);
		assert.doesNotMatch(text, /^error:/mu);
	});

	it('rejects malformed contracts and reuses a linked layer table', () => {
		const layers = '## Layers\n\n| Layer |\n| --- |\n| `base` |\n| `top` |\n';
		const malformed = parseLayerContract(
			`${layers}\n## Module assignment\n\n| Module | Layer |\n| --- | --- |\n| \`a\` | \`middle\` |\n| \`b\` | \`top\` |\n| \`b\` | \`base\` |\n`,
			'malformed.md',
		);
		assert.deepEqual(malformed.problems, [
			'malformed.md: `a` uses unknown layer `middle`',
			'malformed.md: duplicate assignment for `b`',
		]);
		const linked = parseLayerContract(
			'## Layers\n\nSee [shared layers](shared.md#layers).\n\n## Module assignment\n\n| Module | Layer |\n| --- | --- |\n| `a` | `top` |\n',
			'linked.md',
			(target) => (target === 'shared.md' ? { markdown: layers, label: 'shared.md' } : undefined),
		);
		assert.deepEqual(linked.problems, []);
		assert.deepEqual([...linked.layers.keys()], ['base', 'top']);
		const missing = parseLayerContract(
			'## Layers\n\nSee [nothing](absent.md).\n\n## Module assignment\n',
			'missing.md',
		);
		assert.deepEqual(missing.problems, [
			'missing.md: "## Layers" has neither a layer table nor a link to a contract with one',
		]);
	});
});
