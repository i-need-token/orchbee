$global:OutputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $global:OutputEncoding
if ($null -eq $global:__OrchbeeDirectoryPrompt) {
    $global:__OrchbeeDirectoryPrompt = $function:prompt
    function global:prompt {
        # PowerShell command status is Boolean, distinct from a child-process exit code.
        $orchbeeSucceeded = $?
        if ($null -ne $global:__OrchbeeRunId) {
            $orchbeeStatus = if ($orchbeeSucceeded) { 0 } else { 1 }
            [Console]::Write("$([char]27)]133;D;$orchbeeStatus`a$([char]27)_orchbee.command.v1;finish;$global:__OrchbeeRunId;$orchbeeStatus$([char]27)\")
            $global:__OrchbeeRunId = $null
        }
        $out = @(& $global:__OrchbeeDirectoryPrompt) -join ' '
        $loc = $ExecutionContext.SessionState.Path.CurrentLocation
        if ($loc.Provider.Name -eq 'FileSystem') {
            try { [Environment]::CurrentDirectory = $loc.ProviderPath } catch {}
        }
        $out
    }
}
