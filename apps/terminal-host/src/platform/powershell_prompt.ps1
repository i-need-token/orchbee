# Process-local prompt evidence. Keep the user's prompt, history and key handlers.
# If the reader cannot be wrapped, no idle marker is emitted and Close stays conservative.
try {
    $orchbeeReader = (Get-Command PSConsoleHostReadLine -CommandType Function -ErrorAction Stop).ScriptBlock
    $orchbeeWrappedReader = {
        [Console]::Write("$([char]27)]133;A`a$([char]27)]133;B`a")
        try {
            & $orchbeeReader @args
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
