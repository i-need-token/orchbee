$global:OutputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $global:OutputEncoding
if ($null -eq $global:__OrchbeeDirectoryPrompt) {
    $global:__OrchbeeDirectoryPrompt = $function:prompt
    function global:prompt {
        # Capture status during binding; an assignment before the original prompt resets $?.
        param($orchbeeSucceeded = $?)
        $out = @(& $global:__OrchbeeDirectoryPrompt) -join ' '
        if ($null -ne $global:__OrchbeeRunId) {
            $orchbeeStatus = if ($orchbeeSucceeded) { 0 } else { 1 }
            [Console]::Write("$([char]27)]133;D;$orchbeeStatus`a$([char]27)_orchbee.command.v1;finish;$global:__OrchbeeRunId;$orchbeeStatus$([char]27)\")
            $global:__OrchbeeRunId = $null
            Remove-Item -LiteralPath Env:ORCHBEE_RUN_TOKEN -ErrorAction Ignore
        }
        $loc = $ExecutionContext.SessionState.Path.CurrentLocation
        if ($loc.Provider.Name -eq 'FileSystem') {
            try { [Environment]::CurrentDirectory = $loc.ProviderPath } catch {}
        }
        $out
    }
}
