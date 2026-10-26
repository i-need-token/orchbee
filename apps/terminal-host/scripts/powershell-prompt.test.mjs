import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import test from 'node:test';

const bootstrap = readFileSync(
	new URL('../../../crates/local-runtime/src/directory/windows/reader.ps1', import.meta.url),
	'utf8',
);
const shells =
	process.platform === 'win32'
		? [
				path.join(process.env.ProgramFiles ?? 'C:/Program Files', 'PowerShell/7/pwsh.exe'),
				path.join(
					process.env.SystemRoot ?? 'C:/Windows',
					'System32/WindowsPowerShell/v1.0/powershell.exe',
				),
			]
		: [];

for (const executable of shells) {
	test(
		`custom readers preserve hooks, output and exceptions without idle proof in ${path.basename(path.dirname(executable))}`,
		{ skip: !existsSync(executable) },
		() => {
			const encoded = Buffer.from(bootstrap, 'utf16le').toString('base64');
			const script = `
$ErrorActionPreference = 'Stop'
Import-Module PSReadLine
$history = (Get-PSReadLineOption).AddToHistoryHandler
function global:prompt { 'custom prompt' }
$prompt = (Get-Command prompt).ScriptBlock.ToString()
function global:PSConsoleHostReadLine { param($value) "reader:$value" }
$custom = (Get-Command PSConsoleHostReadLine).ScriptBlock
$console = [Console]::Out
$writer = New-Object System.IO.StringWriter
$global:OrchbeeReaderContinuations = 0
[Console]::SetOut($writer)
try {
    $bootstrap = [ScriptBlock]::Create([Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('${encoded}')) + '; $global:OrchbeeReaderContinuations += 1')
    & $bootstrap
    $unchanged = [Object]::ReferenceEquals($custom, (Get-Command PSConsoleHostReadLine).ScriptBlock)
    $value = PSConsoleHostReadLine sample
    $first = $writer.ToString()
    $writer.GetStringBuilder().Clear() | Out-Null
    function global:PSConsoleHostReadLine { throw 'reader failure' }
    & $bootstrap
    $failed = $false
    try { PSConsoleHostReadLine } catch { $failed = $_.Exception.Message -eq 'reader failure' }
    $second = $writer.ToString()
}
finally { [Console]::SetOut($console) }
[ordered]@{
    value = $value
    first = $first
    second = $second
    failed = $failed
    prompt = (Get-Command prompt).ScriptBlock.ToString() -eq $prompt
    history = [Object]::ReferenceEquals((Get-PSReadLineOption).AddToHistoryHandler, $history)
    unchanged = $unchanged
    continuations = $global:OrchbeeReaderContinuations
} | ConvertTo-Json -Compress
`;
			const result = spawnSync(
				executable,
				[
					'-NoLogo',
					'-NoProfile',
					'-NonInteractive',
					'-EncodedCommand',
					Buffer.from(script, 'utf16le').toString('base64'),
				],
				{ encoding: 'utf8', timeout: 15_000, windowsHide: true },
			);
			assert.equal(result.status, 0, result.stderr || String(result.error));
			const value = JSON.parse(result.stdout.trim());
			assert.deepEqual(value, {
				value: 'reader:sample',
				first: '',
				second: '',
				failed: true,
				prompt: true,
				history: true,
				unchanged: true,
				continuations: 2,
			});
		},
	);

	for (const standard of [true, false]) {
		test(
			`${standard ? 'standard PSReadLine reader is wrapped once' : 'custom PSReadLine module definition emits no idle proof'} in ${path.basename(path.dirname(executable))}`,
			{ skip: !existsSync(executable) },
			() => {
				const encoded = Buffer.from(bootstrap, 'utf16le').toString('base64');
				const script = `
$ErrorActionPreference = 'Stop'
Import-Module PSReadLine
$history = (Get-PSReadLineOption).AddToHistoryHandler
function global:prompt { 'custom prompt' }
$prompt = (Get-Command prompt).ScriptBlock.ToString()
${
	standard
		? ''
		: `New-Module -Name PSReadLine -ScriptBlock {
    function PSConsoleHostReadLine { Start-Sleep -Milliseconds 10; 'exit' }
    Export-ModuleMember -Function PSConsoleHostReadLine
} | Import-Module -Global -Force`
}
$before = Get-Command PSConsoleHostReadLine -CommandType Function
$beforeScriptBlock = $before.ScriptBlock
$identity = $before.ModuleName -ceq 'PSReadLine' -and [Object]::ReferenceEquals($before.Module, $before.ScriptBlock.Module) -and [Object]::ReferenceEquals($before.ScriptBlock, $before.Module.ExportedFunctions['PSConsoleHostReadLine'].ScriptBlock)
$console = [Console]::Out
$writer = New-Object System.IO.StringWriter
$global:OrchbeeReaderContinuations = 0
[Console]::SetOut($writer)
try {
    $bootstrap = [ScriptBlock]::Create([Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('${encoded}')) + '; $global:OrchbeeReaderContinuations += 1')
    & $bootstrap
    $after = (Get-Command PSConsoleHostReadLine).ScriptBlock
    & $bootstrap
    $once = [Object]::ReferenceEquals($after, (Get-Command PSConsoleHostReadLine).ScriptBlock)
    ${standard ? '' : '$value = PSConsoleHostReadLine'}
}
finally { [Console]::SetOut($console) }
[ordered]@{
    identity = $identity
    changed = -not [Object]::ReferenceEquals($beforeScriptBlock, $after)
    once = $once
    output = $writer.ToString()
    value = $value
    prompt = (Get-Command prompt).ScriptBlock.ToString() -eq $prompt
    history = [Object]::ReferenceEquals((Get-PSReadLineOption).AddToHistoryHandler, $history)
    continuations = $global:OrchbeeReaderContinuations
} | ConvertTo-Json -Compress
`;
				const result = spawnSync(
					executable,
					[
						'-NoLogo',
						'-NoProfile',
						'-NonInteractive',
						'-EncodedCommand',
						Buffer.from(script, 'utf16le').toString('base64'),
					],
					{ encoding: 'utf8', timeout: 15_000, windowsHide: true },
				);
				assert.equal(result.status, 0, result.stderr || String(result.error));
				assert.deepEqual(JSON.parse(result.stdout.trim()), {
					identity: true,
					changed: standard,
					once: true,
					output: '',
					value: standard ? null : 'exit',
					prompt: true,
					history: true,
					continuations: 2,
				});
			},
		);
	}
}
