# Process-local prompt evidence. Keep the user's prompt, history and key handlers.
# If the reader cannot be wrapped, no idle marker is emitted and Close stays conservative.
& {
    try {
        $orchbeeReaderInfo = Get-Command PSConsoleHostReadLine -CommandType Function -ErrorAction Stop
        $orchbeeReader = $orchbeeReaderInfo.ScriptBlock
        $orchbeeReaderModule = $orchbeeReaderInfo.Module
        if ($orchbeeReaderInfo.ModuleName -cne 'PSReadLine' -or
            $null -eq $orchbeeReaderModule -or
            -not [Object]::ReferenceEquals($orchbeeReaderModule, $orchbeeReader.Module) -or
            -not [Object]::ReferenceEquals($orchbeeReader, $orchbeeReaderModule.ExportedFunctions['PSConsoleHostReadLine'].ScriptBlock)) {
            return
        }
        # Closed fingerprints of PSReadLine's standard two-argument (2.0.0) and
        # status-preserving three-argument (2.4.5) wrappers. Only newlines/outer space normalize.
        # An unknown definition, including one exported by a custom module, is not idle proof.
        $orchbeeReaderHash = [Security.Cryptography.SHA256]::Create()
        try {
            $orchbeeReaderDefinition = $orchbeeReader.ToString().Replace("`r`n", "`n").Trim()
            $orchbeeReaderDigest = [Convert]::ToBase64String($orchbeeReaderHash.ComputeHash([Text.Encoding]::UTF8.GetBytes($orchbeeReaderDefinition)))
        }
        finally { $orchbeeReaderHash.Dispose() }
        if (@('iAnIyZJ+j/P8fdpOZzmqhTQ1K3JRBZTXtr+RZ1yWZpw=', 'UYpf5gWrwcMQjSQneB/98ZokHI5GG6aaCS/hD4Mp7GM=') -cnotcontains $orchbeeReaderDigest) {
            return
        }
        $orchbeeStatusAwareReader = $orchbeeReaderDigest -ceq 'UYpf5gWrwcMQjSQneB/98ZokHI5GG6aaCS/hD4Mp7GM='
        $orchbeeReadContext = & $orchbeeReaderModule { $ExecutionContext }
        $orchbeeWrappedReader = {
            [System.Diagnostics.DebuggerHidden()]
            param()
            # Save status before marker writes; the modern original would recapture their success.
            $orchbeeLastStatus = $?
            Microsoft.PowerShell.Core\Set-StrictMode -Off
            # The prompt is already drawn. P marks this boundary without A's fresh-line effect.
            [Console]::Write("$([char]27)]133;P`a$([char]27)]133;B`a")
            try {
                if ($orchbeeStatusAwareReader) {
                    [Microsoft.PowerShell.PSConsoleReadLine]::ReadLine($host.Runspace, $orchbeeReadContext, $orchbeeLastStatus)
                }
                else {
                    & $orchbeeReader @args
                }
            }
            finally {
                [Console]::Write("$([char]27)]133;C`a")
            }
        }.GetNewClosure()
        Set-Item -LiteralPath Function:\global:PSConsoleHostReadLine -Value $orchbeeWrappedReader -ErrorAction Stop
    }
    catch {
        # Missing/custom readers or restricted language leave process state unknown.
    }
}
