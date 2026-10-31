& {
try {
	$info = Get-Command PSConsoleHostReadLine -CommandType Function -ErrorAction Stop
	$reader = $info.ScriptBlock
	$module = $info.Module
	if ($info.ModuleName -cne 'PSReadLine' -or
		$null -eq $module -or
		-not [Object]::ReferenceEquals($module, $reader.Module) -or
		-not [Object]::ReferenceEquals($reader, $module.ExportedFunctions['PSConsoleHostReadLine'].ScriptBlock)) {
		return
	}
	$hash = [Security.Cryptography.SHA256]::Create()
	try { $digest = [Convert]::ToBase64String($hash.ComputeHash([Text.Encoding]::UTF8.GetBytes($reader.ToString().Replace("`r`n", "`n").Trim()))) }
	finally { $hash.Dispose() }
	$aware = $digest -ceq 'UYpf5gWrwcMQjSQneB/98ZokHI5GG6aaCS/hD4Mp7GM='
	if (-not $aware -and $digest -cne 'iAnIyZJ+j/P8fdpOZzmqhTQ1K3JRBZTXtr+RZ1yWZpw=') {
		return
	}
	$context = & $module { $ExecutionContext }
	$e = [char]27
	$wrapped = {
		[System.Diagnostics.DebuggerHidden()]
		param()
		$status = $?
		Microsoft.PowerShell.Core\Set-StrictMode -Off
		[Console]::Write("$e]133;P`a$e]133;B`a${e}_orchbee.command.v1;ready$e\")
		$options = [Microsoft.PowerShell.PSConsoleReadLine]::GetOptions()
		$history = $options.AddToHistoryHandler
		$options.AddToHistoryHandler = { param($line) if ($line -cmatch '\A<#orchbee-run:[0-9a-f]{32}#> ') { $false } elseif ($history) { $history.Invoke($line) } else { $true } }.GetNewClosure()
		$line = $null
		try {
			$line = if ($aware) { [Microsoft.PowerShell.PSConsoleReadLine]::ReadLine($host.Runspace, $context, $status) } else { & $reader @args }
		}
		finally {
			$options.AddToHistoryHandler = $history
			[Console]::Write("$e]133;C`a")
		}
		$id = $null
		if ($line -cmatch '\A<#orchbee-run:([0-9a-f]{32})#> ') {
			$id = $Matches[1]
			$line = $line.Substring($Matches[0].Length)
			$global:__OrchbeeRunLine = $line
		}
		elseif (-not [string]::IsNullOrWhiteSpace($line)) {
			$id = [Guid]::NewGuid().ToString('N')
			$global:__OrchbeeRunLine = $null
		}
		if ($id) {
			$global:__OrchbeeRunId = $id
			$env:ORCHBEE_RUN_TOKEN = $id
			[Console]::Write("${e}_orchbee.command.v1;start;$id$e\")
		}
		$line
	}.GetNewClosure()
	Set-Item -LiteralPath Function:\global:PSConsoleHostReadLine -Value $wrapped -ErrorAction Stop
}
catch {
}
}
