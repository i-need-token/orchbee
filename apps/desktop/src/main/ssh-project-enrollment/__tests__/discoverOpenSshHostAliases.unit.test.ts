import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { discoverOpenSshHostAliases } from '../discoverOpenSshHostAliases';

const OVERSIZED_CONFIGURATION_BYTES = 1024 * 1024;

describe('discoverOpenSshHostAliases', () => {
	let root: string;
	let home: string;
	let configuration: string;

	beforeEach(async () => {
		root = await mkdtemp(path.join(tmpdir(), 'orchbee-ssh-host-discovery-'));
		home = path.join(root, 'home');
		configuration = path.join(home, '.ssh', 'config');
		await mkdir(path.dirname(configuration), { recursive: true });
	});

	afterEach(async () => {
		await rm(root, { force: true, recursive: true });
	});

	it('treats a missing user configuration as an empty successful discovery', async () => {
		await expect(discoverOpenSshHostAliases(configuration, home)).resolves.toEqual({
			aliases: [],
			issue: null,
		});
	});

	it('follows bounded Includes and returns only unique concrete Host aliases', async () => {
		await mkdir(path.join(home, '.ssh', 'conf.d'), { recursive: true });
		await writeFile(
			configuration,
			[
				'Host build build-* !build-old',
				'  HostName build.internal',
				'Host=Prod',
				'Include "conf.d/*.conf"',
				'Include ${SSH_EXTRA_CONFIG}',
				'Host -option [pattern] ?',
				'',
			].join('\n'),
		);
		await writeFile(
			path.join(home, '.ssh', 'conf.d', '10-first.conf'),
			'Host alpha PROD\nInclude cycle.conf\n',
		);
		await writeFile(path.join(home, '.ssh', 'conf.d', '20-second.conf'), 'Host "quoted-host"\n');
		await writeFile(
			path.join(home, '.ssh', 'cycle.conf'),
			'Include conf.d/10-first.conf\nHost cycle\n',
		);
		await writeFile(path.join(home, '.ssh', 'extra.conf'), 'Host extra\n');

		await expect(
			discoverOpenSshHostAliases(configuration, home, {
				SSH_EXTRA_CONFIG: path.join(home, '.ssh', 'extra.conf'),
			}),
		).resolves.toEqual({
			aliases: ['alpha', 'build', 'cycle', 'extra', 'Prod', 'quoted-host'],
			issue: null,
		});
	});

	it('retains safe aliases while surfacing malformed and oversized configuration', async () => {
		await writeFile(configuration, "Host safe\nHost 'unterminated\n");
		await expect(discoverOpenSshHostAliases(configuration, home)).resolves.toEqual({
			aliases: ['safe'],
			issue: 'invalid-config',
		});

		await writeFile(configuration, `Host safe\n#${'x'.repeat(OVERSIZED_CONFIGURATION_BYTES)}\n`);
		await expect(discoverOpenSshHostAliases(configuration, home)).resolves.toEqual({
			aliases: [],
			issue: 'truncated',
		});
	});
});
