# Static Orchbee native completion. Load explicitly; no profile is modified.
Register-ArgumentCompleter -Native -CommandName orchbee, orchbee.exe -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    $grammar = ConvertFrom-Json @'
__ORCHBEE_GRAMMAR__
'@
    $words = [System.Collections.Generic.List[string]]::new()
    foreach ($element in @($commandAst.CommandElements | Select-Object -Skip 1)) {
        if ($element.Extent.StartOffset -ge $cursorPosition) { break }
        if ($element.Extent.EndOffset -ge $cursorPosition -and $wordToComplete.Length -gt 0) { break }
        if ($element -is [System.Management.Automation.Language.StringConstantExpressionAst]) {
            $words.Add($element.Value)
        } elseif ($element -is [System.Management.Automation.Language.ConstantExpressionAst] -and $element.Extent.Text -cmatch '^[0-9]+$') {
            # Decimal integer literals are syntax, not an expression to evaluate.
            $words.Add($element.Extent.Text)
        } elseif ($element -is [System.Management.Automation.Language.CommandParameterAst]) {
            $words.Add($element.Extent.Text)
        } else {
            # Variable expansion, subexpressions and scriptblocks are never executed.
            return
        }
    }
    $positionals = [System.Collections.Generic.List[string]]::new()
    $seen = [System.Collections.Generic.List[string]]::new()
    $values = [System.Collections.Generic.Dictionary[string,string]]::new([System.StringComparer]::Ordinal)
    $pending = ''
    foreach ($word in $words) {
        if ($word -ceq '--') { return }
        if ($pending.Length -gt 0) {
            $values[$pending] = $word
            $pending = ''
            continue
        }
        if ($word -cin @('--help', '-h', '--version', '--skill')) { return }
        if ($word.StartsWith('-') -and $word.Length -gt 1) {
            $pair = $word.Split([char[]]@('='), 2, [System.StringSplitOptions]::None)
            $flag = $pair[0]
            if ($grammar.flags -cnotcontains $flag) { return }
            if ($seen -ccontains $flag -and $grammar.repeatable -cnotcontains $flag -and $flag -cne '--provider') { return }
            $seen.Add($flag)
            if ($grammar.valueFlags -ccontains $flag) {
                if ($pair.Length -eq 2) { $values[$flag] = $pair[1] } else { $pending = $flag }
            } elseif ($pair.Length -eq 2) { return }
        } else {
            $positionals.Add($word)
        }
    }
    $candidates = [System.Collections.Generic.List[string]]::new()
    if ($positionals.Count -gt 0 -and $positionals[0] -cin @('completion', 'help')) {
        if ($seen.Count -gt 0) { return }
        if ($positionals[0] -ceq 'completion') {
            if ($positionals.Count -ne 1) { return }
            $candidates.AddRange([string[]]$grammar.shells)
            $candidates.Add('--help')
        } elseif ($positionals.Count -eq 1) {
            $candidates.AddRange([string[]]$grammar.helpTopics)
        } elseif ($positionals.Count -eq 2) {
            foreach ($entry in $grammar.helpChildren) {
                if ($entry[0] -ceq $positionals[1]) { $candidates.AddRange([string[]]$entry[1]) }
            }
        } elseif ($positionals.Count -eq 3) {
            foreach ($entry in $grammar.helpLevel3) {
                if ($entry[0] -ceq ($positionals[1] + ':' + $positionals[2])) { $candidates.AddRange([string[]]$entry[1]) }
            }
        }
    } else {
        foreach ($group in @(@('--timeout', '--forever'), @('--session', '--name', '--panel'), @('--panel', '--current'), @('--workspace', '--standalone'), @('--exec', '--profile'), @('--arg', '--profile'), @('--into-tab', '--new-tab'), @('--target-panel', '--into-tab'), @('--target-panel', '--new-tab'), @('--text', '--file', '--stdin', '--key'), @('--key', '--enter'), @('--key', '--paste'), @('--text', '--paste'), @('--match', '--regex'))) {
            if (@($group | Where-Object { $seen -ccontains $_ }).Count -gt 1) { return }
        }
        foreach ($entry in @(@('--format', $grammar.formats), @('--mouse', @('off', 'on')), @('--until', $grammar.until), @('--right-click', $grammar.rightClick), @('--source', $grammar.outputSources))) {
            if ($values.ContainsKey($entry[0]) -and $entry[1] -cnotcontains $values[$entry[0]]) { return }
        }
        $viable = @($grammar.commands | Where-Object {
            $command = $_
            $allowed = @($command.flags)
            if ($command.words[0] -cne 'tui') { $allowed += '--format' }
            $valid = $true
            $commandPath = $command.words -join ':'
            if ($command.words[0] -ceq 'worktree' -and $seen -ccontains '--workspace' -and $seen -ccontains '--cwd') { $valid = $false }
            if ($commandPath -ceq 'worktree:open' -and $seen -ccontains '--path' -and $seen -ccontains '--branch') { $valid = $false }
            if ($commandPath -ceq 'worktree:list' -and ($seen -ccontains '--project' -or $seen -ccontains '--directory') -and ($seen -ccontains '--workspace' -or $seen -ccontains '--cwd' -or $seen -ccontains '--trust-repository')) { $valid = $false }
            foreach ($pair in $command.conflicts) {
                if ($seen -ccontains $pair.flag -and @($pair.with | Where-Object { $seen -ccontains $_ }).Count -gt 0) { $valid = $false }
            }
            foreach ($flag in $seen) { if ($allowed -cnotcontains $flag) { $valid = $false } }
            if (@($seen | Where-Object { $_ -ceq '--provider' }).Count -gt 1 -and ($command.words -join ':') -cne 'agent:doctor') { $valid = $false }
            $operandLimit = [int]$command.operand
            if ($commandPath -ceq 'panel:process-info' -and $seen -ccontains '--client') { $operandLimit = 0 }
            if (($command.words -join ':') -cin @('panel:process-info', 'panel:focus', 'panel:zoom', 'panel:layout', 'panel:neighbor', 'panel:edges', 'panel:resize', 'panel:swap', 'panel:input', 'panel:split') -and ($seen -ccontains '--panel' -or $seen -ccontains '--current')) { $operandLimit = 0 }
            if (($command.words -join ':') -cin @('agent:get', 'agent:focus', 'agent:read', 'agent:send-keys', 'agent:prompt', 'agent:explain', 'agent:wait', 'agent:resolve', 'agent:rename')) {
                if ($seen -ccontains '--session' -or $seen -ccontains '--name' -or $seen -ccontains '--panel') { $operandLimit-- }
                if ($seen -ccontains '--clear') { $operandLimit-- }
            }
            if ($positionals.Count -gt $command.words.Count + $operandLimit) { $valid = $false }
            for ($index = 0; $index -lt [Math]::Min($positionals.Count, $command.words.Count); $index++) {
                if ($positionals[$index] -cne $command.words[$index]) { $valid = $false }
            }
            if ($values.ContainsKey('--direction') -and $command.directions -cnotcontains $values['--direction']) { $valid = $false }
            $valid
        })
        if ($viable.Count -eq 0) { return }
        $offeredFlags = [System.Collections.Generic.List[string]]::new()
        foreach ($command in $viable) {
            if ($positionals.Count -lt $command.words.Count) { $candidates.Add($command.words[$positionals.Count]) }
            if (($command.words -join ':') -ceq 'agent:send-keys') {
                $targetWords = [int](-not ($seen -ccontains '--session' -or $seen -ccontains '--name' -or $seen -ccontains '--panel'))
                $keyCount = $positionals.Count - $command.words.Count - $targetWords
                if ($keyCount -ge 0 -and $keyCount -lt 64) { foreach ($key in $grammar.keys) { $candidates.Add($key) } }
            }
            $allowed = @($command.flags)
            if ($command.words[0] -cne 'tui') { $allowed += '--format' }
            foreach ($flag in $allowed) {
                if (($command.words -join ':') -ceq 'panel:process-info' -and $positionals.Count -gt $command.words.Count -and $flag -ceq '--client') { continue }
                if (($command.words -join ':') -cin @('panel:process-info', 'panel:focus', 'panel:zoom', 'panel:layout', 'panel:neighbor', 'panel:edges', 'panel:resize', 'panel:swap', 'panel:input', 'panel:split') -and $positionals.Count -gt $command.words.Count -and $flag -cin @('--panel', '--current')) { continue }
                if (($command.words -join ':') -cin @('agent:get', 'agent:focus', 'agent:read', 'agent:send-keys', 'agent:prompt', 'agent:explain', 'agent:wait', 'agent:resolve', 'agent:rename')) {
                    $operands = $positionals.Count - $command.words.Count
                    if ($operands -gt 0 -and $flag -cin @('--name', '--session', '--panel')) { continue }
                    if ($flag -ceq '--clear' -and ($operands -gt 1 -or ($operands -gt 0 -and ($seen -ccontains '--name' -or $seen -ccontains '--session' -or $seen -ccontains '--panel')))) { continue }
                }
                if ($seen -ccontains $flag -and $grammar.repeatable -cnotcontains $flag -and -not ($flag -ceq '--provider' -and ($command.words -join ':') -ceq 'agent:doctor')) { continue }
                if ($flag -ceq '--arg' -and $seen -cnotcontains '--exec' -and ($command.words -join ':') -cne 'agent:start') { continue }
                $conflict = $false
                foreach ($pair in $command.conflicts) {
                    if ($pair.flag -ceq $flag -and @($pair.with | Where-Object { $seen -ccontains $_ }).Count -gt 0) { $conflict = $true }
                }
                if (($command.words -join ':') -ceq 'worktree:list') {
                    if (@('--project', '--directory') -ccontains $flag -and ($seen -ccontains '--workspace' -or $seen -ccontains '--cwd' -or $seen -ccontains '--trust-repository')) { $conflict = $true }
                    if (@('--workspace', '--cwd', '--trust-repository') -ccontains $flag -and ($seen -ccontains '--project' -or $seen -ccontains '--directory')) { $conflict = $true }
                }
                if ($command.words[0] -ceq 'worktree' -and (($flag -ceq '--workspace' -and $seen -ccontains '--cwd') -or ($flag -ceq '--cwd' -and $seen -ccontains '--workspace'))) { $conflict = $true }
                if (($command.words -join ':') -ceq 'worktree:open' -and (($flag -ceq '--path' -and $seen -ccontains '--branch') -or ($flag -ceq '--branch' -and $seen -ccontains '--path'))) { $conflict = $true }
                foreach ($group in @(@('--timeout', '--forever'), @('--session', '--name', '--panel'), @('--panel', '--current'), @('--workspace', '--standalone'), @('--exec', '--profile'), @('--arg', '--profile'), @('--into-tab', '--new-tab'), @('--target-panel', '--into-tab'), @('--target-panel', '--new-tab'), @('--text', '--file', '--stdin', '--key'), @('--key', '--enter'), @('--key', '--paste'), @('--text', '--paste'), @('--match', '--regex'))) {
                    if ($group -ccontains $flag -and @($group | Where-Object { $_ -cne $flag -and $seen -ccontains $_ }).Count -gt 0) { $conflict = $true }
                }
                if (-not $conflict) { $offeredFlags.Add($flag) }
            }
            if ($seen.Count -eq 0 -and $positionals.Count -le $command.words.Count) { $candidates.Add('--help') }
        }
        $joinedPrefix = ''
        if ($pending.Length -eq 0 -and $wordToComplete -cmatch '^(--[^=]+)=') {
            $joinedPrefix = $Matches[1] + '='
            if ($offeredFlags -cnotcontains $Matches[1]) { return }
            $pending = $Matches[1]
        }
        if ($pending.Length -gt 0) {
            $candidates.Clear()
            $enum = switch -CaseSensitive ($pending) {
                '--format' { $grammar.formats }
                '--mouse' { 'off'; 'on' }
                '--cwd' { $viable | ForEach-Object { $_.cwdValues } }
                '--direction' { $viable | ForEach-Object { $_.directions } }
                '--right-click' { $grammar.rightClick }
                '--source' { $grammar.outputSources }
                '--key' { $grammar.keys }
                '--until' { $grammar.until }
                '--provider' { $grammar.providers }
                '--kind' { $grammar.agentKinds }
            }
            foreach ($value in $enum) { $candidates.Add($joinedPrefix + $value) }
        } else {
            $candidates.AddRange($offeredFlags)
            if ($positionals.Count -eq 0 -and $seen.Count -eq 0) {
                $candidates.AddRange([string[]]@('help', 'completion', '--version', '--skill'))
            }
        }
    }
    $candidates | Sort-Object -Unique -CaseSensitive | Where-Object {
        $_.StartsWith($wordToComplete, [System.StringComparison]::Ordinal)
    } | ForEach-Object {
        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
    }
}
