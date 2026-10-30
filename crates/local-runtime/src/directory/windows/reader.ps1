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
        $orchbeeLastStatus = $?
        Microsoft.PowerShell.Core\Set-StrictMode -Off
        [Console]::Write("$([char]27)]133;P`a$([char]27)]133;B`a$([char]27)_orchbee.command.v1;ready$([char]27)\")
        $orchbeeLine = $null
        try {
            if ($orchbeeStatusAwareReader) {
                $orchbeeLine = [Microsoft.PowerShell.PSConsoleReadLine]::ReadLine($host.Runspace, $orchbeeReadContext, $orchbeeLastStatus)
            }
            else {
                $orchbeeLine = & $orchbeeReader @args
            }
        }
        finally {
            [Console]::Write("$([char]27)]133;C`a")
        }
        if ($orchbeeLine -cmatch '\A<#orchbee-run:([0-9a-f]{32})#> ') {
            $global:__OrchbeeRunId = $Matches[1]
            $global:__OrchbeeRunTyped = $false
            $env:ORCHBEE_RUN_TOKEN = $global:__OrchbeeRunId
            $orchbeeLine = $orchbeeLine.Substring($Matches[0].Length)
            [Console]::Write("$([char]27)_orchbee.command.v1;start;$global:__OrchbeeRunId$([char]27)\")
        }
        elseif (-not [string]::IsNullOrWhiteSpace($orchbeeLine)) {
            # A typed command gets its own shell-minted run boundary; OSC 133;D stays absent.
            $global:__OrchbeeRunId = [Guid]::NewGuid().ToString('N')
            $global:__OrchbeeRunTyped = $true
            $env:ORCHBEE_RUN_TOKEN = $global:__OrchbeeRunId
            [Console]::Write("$([char]27)_orchbee.command.v1;start;$global:__OrchbeeRunId$([char]27)\")
        }
        $orchbeeLine
    }.GetNewClosure()
    Set-Item -LiteralPath Function:\global:PSConsoleHostReadLine -Value $orchbeeWrappedReader -ErrorAction Stop
}
catch {
}
}
