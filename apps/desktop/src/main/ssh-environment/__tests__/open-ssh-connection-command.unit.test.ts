import { describe, expect, it } from 'vitest';

import {
	createOpenSshTargetOptionArguments,
	formatOpenSshConnectionCommand,
	parseOpenSshConnectionCommand,
} from '../open-ssh-connection-command';

describe('OpenSSH connection command', () => {
	it('accepts a plain destination and the optional ssh executable prefix', () => {
		expect(parseOpenSshConnectionCommand('alice@192.0.2.10')).toEqual({
			configurationFilePath: null,
			destination: 'alice@192.0.2.10',
		});
		expect(parseOpenSshConnectionCommand('ssh alice@192.0.2.10')).toEqual({
			configurationFilePath: null,
			destination: 'alice@192.0.2.10',
		});
		expect(parseOpenSshConnectionCommand('ssh')).toEqual({
			configurationFilePath: null,
			destination: 'ssh',
		});
	});

	it('parses supported connection options before or after the destination', () => {
		const target = parseOpenSshConnectionCommand(
			'ssh -A -C -p 2222 alice@192.0.2.10 -i ~/.ssh/id_ed25519 -J bastion',
		);
		expect(target).toEqual({
			configurationFilePath: null,
			destination: 'alice@192.0.2.10',
			options: [
				{ enabled: true, kind: 'forward-agent' },
				{ enabled: true, kind: 'compression' },
				{ kind: 'identity-file', value: '~/.ssh/id_ed25519' },
				{ kind: 'proxy-jump', value: 'bastion' },
				{ kind: 'port', value: 2222 },
			],
		});
		expect(createOpenSshTargetOptionArguments(target.options)).toEqual([
			'-o',
			'ForwardAgent=yes',
			'-o',
			'Compression=yes',
			'-i',
			'~/.ssh/id_ed25519',
			'-J',
			'bastion',
			'-p',
			'2222',
		]);
	});

	it('normalizes supported -o settings and expands a user configuration path', () => {
		expect(
			parseOpenSshConnectionCommand(
				'ssh -F ~/.ssh/work -o User=alice -oPort=2200 -o "ForwardAgent no" build',
				{ homeDirectory: '/Users/alice' },
			),
		).toEqual({
			configurationFilePath: '/Users/alice/.ssh/work',
			destination: 'build',
			options: [
				{ enabled: false, kind: 'forward-agent' },
				{ kind: 'user', value: 'alice' },
				{ kind: 'port', value: 2200 },
			],
		});
	});

	it('normalizes semantically equivalent option order while preserving identity-file order', () => {
		const first = parseOpenSshConnectionCommand(
			'ssh -p 2222 -i first-key -A -i second-key alice@192.0.2.10',
		);
		const second = parseOpenSshConnectionCommand(
			'ssh -A -i first-key -i second-key -p 2222 alice@192.0.2.10',
		);
		expect(first).toEqual(second);
		expect(first.options).toEqual([
			{ enabled: true, kind: 'forward-agent' },
			{ kind: 'identity-file', value: 'first-key' },
			{ kind: 'identity-file', value: 'second-key' },
			{ kind: 'port', value: 2222 },
		]);
	});

	it('formats a stable command that parses back to the same target', () => {
		const target = parseOpenSshConnectionCommand(
			"ssh -i '/Users/example/SSH Keys/id_ed25519' -p 2200 host",
		);
		const formatted = formatOpenSshConnectionCommand(target);
		expect(formatted).toBe("ssh -i '/Users/example/SSH Keys/id_ed25519' -p 2200 host");
		expect(parseOpenSshConnectionCommand(formatted)).toEqual(target);
	});

	it.each([
		'ssh -A',
		'ssh -L 9000:localhost:9000 host',
		'ssh host uname',
		'ssh host;rm',
		'ssh -p 22 -p 23 host',
		'ssh -o ProxyCommand=helper host',
		'ssh -F relative/config host',
	])('rejects unsupported or ambiguous command input %s', (input) => {
		expect(() => parseOpenSshConnectionCommand(input)).toThrow(Error);
	});
});
