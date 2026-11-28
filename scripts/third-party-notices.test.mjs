import assert from 'node:assert/strict';
import { mkdtempSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { readInventory, readLicenseTexts } from './third-party-notices.mjs';

test('the inventory and the supplied license texts give every distributed component its text', () => {
	const sections = readInventory();
	assert.ok(sections.length > 0);
	for (const section of sections)
		if (section.carriers.some((carrier) => carrier.distribution !== undefined))
			assert.ok(section.texts.length > 0 || section.none !== undefined, section.name);
	assert.ok(readLicenseTexts().size > 0);
});

test('a distributed component without a license text stops packaging, naming it', (t) => {
	const parent = realpathSync(tmpdir());
	const root = mkdtempSync(path.join(parent, 'orchbee-notices-test-'));
	t.after(() => {
		const actual = realpathSync(root);
		assert.equal(path.dirname(actual), parent);
		assert.ok(path.basename(actual).startsWith('orchbee-notices-test-'));
		rmSync(actual, { recursive: true });
	});
	writeFileSync(
		path.join(root, 'THIRD_PARTY_NOTICES.md'),
		[
			'# Third-Party Notices',
			'',
			'## Quoted',
			'',
			'- Carried by: native programs on Linux',
			'',
			'> Permission is granted.',
			'',
			'## Reasoned',
			'',
			'- Carried by: Desktop application',
			'- License text: none; in the public domain',
			'',
			'## Website only',
			'',
			'- Carried by: website',
			'',
			'## Silent',
			'',
			'- License: MIT',
			'- Carried by: native programs in development and test builds',
			'',
			'## Unlinked',
			'',
			'- Carried by: Desktop application on Windows',
			'- License text: [`LICENSE`](missing/LICENSE)',
			'',
			'## Elsewhere',
			'',
			'- Carried by: the server',
			'',
		].join('\n'),
	);
	assert.throws(
		() => readInventory(root),
		(error) => {
			assert.match(error.message, /packaging stops/u);
			assert.match(error.message, /section "Silent" has no license text/u);
			assert.match(error.message, /section "Unlinked" links missing\/LICENSE/u);
			assert.match(error.message, /section "Elsewhere" is carried by "the server"/u);
			assert.doesNotMatch(error.message, /Quoted|Reasoned|Website only/u);
			return true;
		},
	);
});
