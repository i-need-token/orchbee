$global:OutputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $global:OutputEncoding
$global:__OrchbeeHistoryId = $MyInvocation.HistoryId
if ($null -eq $global:__OrchbeeDirectoryPrompt) {
	$global:__OrchbeeDirectoryPrompt = $function:prompt
	function global:prompt {
		param($orchbeeSucceeded = $?)
		$out = @(& $global:__OrchbeeDirectoryPrompt) -join ' '
		if ($global:__OrchbeeHistoryId -gt 0) { Microsoft.PowerShell.Core\Clear-History -Id $global:__OrchbeeHistoryId -ErrorAction Ignore; $global:__OrchbeeHistoryId = 0 }
		if ($null -ne $global:__OrchbeeRunId) {
			$e = [char]27
			$status = if ($orchbeeSucceeded) { 0 } else { 1 }
			$end = ''
			if ($null -ne $global:__OrchbeeRunLine) {
				$end = "$e]133;D;$status`a"
				$last = Microsoft.PowerShell.Core\Get-History -Count 1
				if ($last.CommandLine -ceq $global:__OrchbeeRunLine) { Microsoft.PowerShell.Core\Clear-History -Id $last.Id }
			}
			[Console]::Write("$end${e}_orchbee.command.v1;finish;$global:__OrchbeeRunId;$status$e\")
			$global:__OrchbeeRunId = $null
			$env:ORCHBEE_RUN_TOKEN = $null
		}
		$loc = $ExecutionContext.SessionState.Path.CurrentLocation
		if ($loc.Provider.Name -eq 'FileSystem') {
			try { [Environment]::CurrentDirectory = $loc.ProviderPath } catch {}
		}
		$out
	}
}
