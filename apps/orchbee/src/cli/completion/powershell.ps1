# Static Orchbee native completion. Load explicitly; no profile is modified.
Register-ArgumentCompleter -Native -CommandName orchbee, orchbee.exe -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    $grammar = ConvertFrom-Json @'
__ORCHBEE_GRAMMAR__
'@
    $words = [System.Collections.Generic.List[string]]::new()
    $currentElement = $null
    foreach ($element in @($commandAst.CommandElements | Select-Object -Skip 1)) {
        if ($element.Extent.StartOffset -ge $cursorPosition) { break }
        if ($element.Extent.EndOffset -ge $cursorPosition -and $wordToComplete.Length -gt 0) {
            $currentElement = $element
            break
        }
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
    # PowerShell parses an unquoted @agent:... token as a variable/splat AST
    # and does not invoke native completion for it. Keep that host boundary.
    if ($currentElement -is [System.Management.Automation.Language.VariableExpressionAst]) { return }
    $completionPrefix = $wordToComplete
    if ($currentElement -is [System.Management.Automation.Language.StringConstantExpressionAst]) {
        # Native completion receives the extent including its quotes. Match
        # decoded text, then replace the whole token with a safe single-quoted
        # target so @ is never interpreted as splatting.
        $completionPrefix = $currentElement.Value
    }
    $machine = $false
    if ($words.Count -gt 0 -and $grammar.prefixFlags -ccontains '--machine') {
        if ($words[0] -ceq '--machine') {
            if ($words.Count -lt 2) { return }
            $selector = $words[1]
            $words.RemoveRange(0, 2)
            $machine = $true
        } elseif ($words[0].StartsWith('--machine=')) {
            $selector = $words[0].Substring(10)
            $words.RemoveAt(0)
            $machine = $true
        }
        if ($machine -and ($selector.Trim().Length -eq 0 -or $selector.StartsWith('-'))) { return }
    }
    $positionals = [System.Collections.Generic.List[string]]::new()
    $seen = [System.Collections.Generic.List[string]]::new()
    $values = [System.Collections.Generic.Dictionary[string,string]]::new([System.StringComparer]::Ordinal)
    $allValues = [System.Collections.Generic.List[object]]::new()
    $pending = ''
    foreach ($word in $words) {
        if ($word -ceq '--') { return }
        if ($pending.Length -gt 0) {
            $values[$pending] = $word
            $allValues.Add(@($pending, $word))
            $pending = ''
            continue
        }
        if (($grammar.rootModeFlags -ccontains $word) -or $word -ceq '-h') { return }
        if ($word.StartsWith('-') -and $word.Length -gt 1) {
            $pair = $word.Split([char[]]@('='), 2, [System.StringSplitOptions]::None)
            $flag = $pair[0]
            if ($grammar.flags -cnotcontains $flag) { return }
            if ($seen -ccontains $flag -and $grammar.repeatable -cnotcontains $flag) { return }
            if ($pair.Length -eq 2 -and $grammar.joinedFlags -cnotcontains $flag) { return }
            $seen.Add($flag)
            if ($grammar.valueFlags -ccontains $flag) {
                if ($pair.Length -eq 2) { $values[$flag] = $pair[1]; $allValues.Add(@($flag, $pair[1])) } else { $pending = $flag }
            } elseif ($pair.Length -eq 2) { return }
        } else {
            $positionals.Add($word)
        }
    }
    function Test-OrchbeeLiteral($value, $rule) {
        if ($null -eq $rule) { return $true }
        if ($rule.trim) { $value = $value.Trim($grammar.trimCharacters.ToCharArray()) }
        if ($value.Length -lt $rule.minimum -or ($null -ne $rule.maximum -and $value.Length -gt $rule.maximum)) { return $false }
        return (($null -eq $rule.allowed -or $value -cnotmatch ('[^' + $rule.allowed + ']')) -and $rule.excluded -cnotcontains $value)
    }
    if ($machine -and $seen -ccontains '--session') { return }
    $businessSeen = @($seen | Where-Object { $grammar.globalFlags -cnotcontains $_ })
    $candidates = [System.Collections.Generic.List[string]]::new()
    if ($positionals.Count -gt 0 -and $positionals[0] -cin @('completion', 'help')) {
        if ($businessSeen.Count -gt 0 -or $pending.Length -gt 0) { return }
        if ($positionals[0] -ceq 'completion') {
            if ($machine) { return }
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
        $viable = @($grammar.commands | Where-Object {
            $command = $_
            if ($machine -and -not $command.machine) { return $false }
            $allowed = @($command.flags) + @($command.globalFlags)
            if ($command.format) { $allowed += '--format' }
            $valid = $true
            foreach ($value in $command.values) {
                foreach ($occurrence in $allValues) {
                    if ($occurrence[0] -cne $value.flag) { continue }
                    if ($value.closed -and $value.choices -cnotcontains $occurrence[1]) { $valid = $false }
                    if (-not (Test-OrchbeeLiteral $occurrence[1] $value.string)) { $valid = $false }
                }
            }
            foreach ($pair in $command.conflicts) {
                if ($seen -ccontains $pair.flag -and @($pair.with | Where-Object { $seen -ccontains $_ }).Count -gt 0) { $valid = $false }
            }
            foreach ($flag in $seen) { if ($allowed -cnotcontains $flag) { $valid = $false } }
            foreach ($flag in $seen) {
                if (@($seen | Where-Object { $_ -ceq $flag }).Count -gt 1 -and $command.repeatable -cnotcontains $flag) { $valid = $false }
            }
            $operandLimit = [int]$command.operand
            foreach ($group in $command.reductions) {
                if (@($group | Where-Object { $seen -ccontains $_ }).Count -gt 0) { $operandLimit-- }
            }
            if ($positionals.Count -gt $command.words.Count + $operandLimit) { $valid = $false }
            if ($positionals.Count -gt $command.words.Count -and $null -ne $command.operandString) {
                $operand = $positionals[$command.words.Count]
                if (-not (Test-OrchbeeLiteral $operand $command.operandString)) { $valid = $false }
                if ($values.ContainsKey($command.operandEquals) -and $values[$command.operandEquals] -cne $operand) { $valid = $false }
            }
            for ($index = 0; $index -lt [Math]::Min($positionals.Count, $command.words.Count); $index++) {
                if ($positionals[$index] -cne $command.words[$index]) { $valid = $false }
            }
            $valid
        })
        if ($viable.Count -eq 0) { return }
        $offeredFlags = [System.Collections.Generic.List[string]]::new()
        foreach ($command in $viable) {
            if ($positionals.Count -lt $command.words.Count) { $candidates.Add($command.words[$positionals.Count]) }
            $selected = @($command.selectors | Where-Object { $seen -ccontains $_ }).Count -gt 0
            if ($command.agentTarget -and $positionals.Count -eq $command.words.Count -and -not $selected) {
                $candidates.AddRange([string[]]$grammar.agentTargets)
            }
            $offset = [int]$command.operandValueOffset - [int]$selected
            $operands = $positionals.Count - $command.words.Count
            $limit = [int]$command.operand
            foreach ($group in $command.reductions) {
                if (@($group | Where-Object { $seen -ccontains $_ }).Count -gt 0) { $limit-- }
            }
            if ($operands -ge $offset -and $operands -lt $limit) { $candidates.AddRange([string[]]$command.operandValues) }
            $allowed = @($command.flags) + @($command.globalFlags)
            if ($command.format) { $allowed += '--format' }
            foreach ($flag in $allowed) {
                if ($machine -and $flag -ceq '--session') { continue }
                if ($operands -gt 0 -and $command.selectors -ccontains $flag) { continue }
                if ($seen -ccontains $flag -and $command.repeatable -cnotcontains $flag) { continue }
                $proposedLimit = [int]$command.operand
                foreach ($group in $command.reductions) {
                    if ($group -ccontains $flag -or @($group | Where-Object { $seen -ccontains $_ }).Count -gt 0) { $proposedLimit-- }
                }
                if ($operands -gt $proposedLimit) { continue }
                $missing = @($command.requirements | Where-Object {
                    $_.flag -ceq $flag -and @($_.any | Where-Object { $seen -ccontains $_ }).Count -eq 0
                }).Count -gt 0
                if ($missing) { continue }
                $conflict = $false
                foreach ($pair in $command.conflicts) {
                    if ($pair.flag -ceq $flag -and @($pair.with | Where-Object { $seen -ccontains $_ }).Count -gt 0) { $conflict = $true }
                }
                if (-not $conflict) { $offeredFlags.Add($flag) }
            }
            $candidates.Add('--help')
        }
        $joinedPrefix = ''
        if ($pending.Length -eq 0 -and $wordToComplete -cmatch '^(--[^=]+)=') {
            $joinedPrefix = $Matches[1] + '='
            if ($offeredFlags -cnotcontains $Matches[1]) { return }
            if ($grammar.joinedFlags -cnotcontains $Matches[1]) { return }
            $pending = $Matches[1]
        }
        if ($pending.Length -gt 0) {
            $candidates.Clear()
            $enum = $viable | ForEach-Object { $_.values } | Where-Object { $_.flag -ceq $pending } | ForEach-Object { $_.choices }
            foreach ($value in $enum) { $candidates.Add($joinedPrefix + $value) }
        } else {
            $candidates.AddRange($offeredFlags)
            if ($positionals.Count -eq 0 -and $businessSeen.Count -eq 0) {
                if ($machine) {
                    $candidates.Add('help')
                } else {
                    $candidates.AddRange([string[]]@('help', 'completion'))
                    foreach ($flag in $grammar.rootModeFlags) {
                        if (@($grammar.rootModeFlagsRequireNoSeenFlags) -ccontains $flag -and $seen.Count -gt 0) { continue }
                        $candidates.Add($flag)
                    }
                }
            }
        }
    }
    if (-not $machine -and $words.Count -eq 0) { $candidates.AddRange([string[]]$grammar.prefixFlags) }
    $candidates | Sort-Object -Unique -CaseSensitive | Where-Object {
        $_.StartsWith($completionPrefix, [System.StringComparison]::Ordinal)
    } | ForEach-Object {
        $completionText = $_
        if ($grammar.agentTargets -ccontains $_) { $completionText = "'" + $_.Replace("'", "''") + "'" }
        [System.Management.Automation.CompletionResult]::new($completionText, $completionText, 'ParameterValue', $completionText)
    }
}
