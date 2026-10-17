import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import test from 'node:test';

const bootstrap = readFileSync(
	new URL('../src/platform/powershell_prompt.ps1', import.meta.url),
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
		`reader markers preserve hooks, output and exceptions in ${path.basename(path.dirname(executable))}`,
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
$console = [Console]::Out
$writer = New-Object System.IO.StringWriter
[Console]::SetOut($writer)
try {
    $bootstrap = [ScriptBlock]::Create([Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('${encoded}')))
    & $bootstrap
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
			const markers = '\x1b]133;A\x07\x1b]133;B\x07\x1b]133;C\x07';
			assert.deepEqual(value, {
				value: 'reader:sample',
				first: markers,
				second: markers,
				failed: true,
				prompt: true,
				history: true,
			});
		},
	);
}
