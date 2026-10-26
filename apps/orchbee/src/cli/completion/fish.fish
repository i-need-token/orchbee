function __orchbee_limit
    set -l key $argv[1]
    set -l proposed $argv[2]
    set -l seen $argv[3..]
    set -l limit (__orchbee_meta "limit|$key")
    for group in (__orchbee_meta "reductions|$key")
        set -l matched 0
        for flag in (string split ' ' -- $group)
            if contains -- $flag $seen; or test "$flag" = "$proposed"
                set matched 1
            end
        end
        set limit (math $limit - $matched)
    end
    printf '%s\n' $limit
end
function __orchbee_string
    set -l key $argv[1]
    set -l value $argv[2]
    set -l pattern (__orchbee_meta "string|$key")
    test (count $pattern) -gt 0; or return 0
    if test (count (__orchbee_meta "trim|$key")) -gt 0
        set value (string trim -- $value)
    end
    contains -- $value (__orchbee_meta "excluded|$key"); and return 1
    string match -qr -- $pattern $value
end
function __orchbee_candidates
    set -l words (commandline -opc)
    set -e words[1]
    set -l prefix (commandline -ct)
    set -l original_count (count $words)
    set -l machine 0
    if test (count $words) -gt 0
        if test "$words[1]" = --machine
            test (count $words) -ge 2; or return
            string match -qr -- '^[^-].*' $words[2]; or return
            set machine 1
            set -e words[1..2]
        else if string match -q -- '--machine=*' $words[1]
            set -l selector (string sub -s 11 -- $words[1])
            string match -qr -- '^[^-].*' $selector; or return
            set machine 1
            set -e words[1]
        end
    end
    set -l positionals
    set -l seen
    set -l value_flags
    set -l values
    set -l pending ''
    set -l modes (__orchbee_meta modes)
    set -l flags (__orchbee_meta flags)
    set -l takes_value (__orchbee_meta value-flags)
    for word in $words
        test "$word" = --; and return
        if test -n "$pending"
            set -a value_flags $pending
            set -a values $word
            set pending ''
            continue
        end
        if contains -- $word $modes; or test "$word" = -h
            return
        end
        if string match -qr -- '^-.+' $word
            set -l pair (string split -m 1 = -- $word)
            set -l flag $pair[1]
            contains -- $flag $flags; or return
            if test (count $pair) -eq 2; and not contains -- $flag (__orchbee_meta joined-flags)
                return
            end
            set -a seen $flag
            if contains -- $flag $takes_value
                if test (count $pair) -eq 2
                    set -a value_flags $flag
                    set -a values $pair[2]
                else
                    set pending $flag
                end
            else if test (count $pair) -eq 2
                return
            end
        else
            set -a positionals $word
        end
    end
    set -l global (__orchbee_meta global)
    set -l business 0
    for flag in $seen
        contains -- $flag $global; or set business 1
    end
    if test $machine -eq 1; and contains -- --session $seen
        return
    end
    set -l candidates
    set -l n (count $positionals)
    if test $n -gt 0; and contains -- $positionals[1] help completion
        test $business -eq 0; and test -z "$pending"; or return
        if test "$positionals[1]" = completion
            test $n -eq 1; and test $machine -eq 0; or return
            set candidates (__orchbee_meta shells) --help
        else if test $n -eq 1
            set candidates (__orchbee_meta root)
        else
            set -l help_key (string join : -- $positionals[2..])
            set candidates (__orchbee_meta "help|$help_key")
        end
    else
        set -l viable
        set -l offered
        for key in (__orchbee_meta commands)
            set -l path (__orchbee_meta "words|$key")
            set -l width (count $path)
            set -l valid 1
            set -l index 1
            while test $index -le $width; and test $index -le $n
                if test "$path[$index]" != "$positionals[$index]"
                    set valid 0
                    break
                end
                set index (math $index + 1)
            end
            test $valid -eq 1; or continue
            if test $machine -eq 1
                test (count (__orchbee_meta "machine|$key")) -gt 0; or continue
            end
            set -l allowed (__orchbee_meta "flags|$key")
            set -l repeats (__orchbee_meta "repeat|$key")
            for flag in $seen
                if not contains -- $flag $allowed
                    set valid 0
                    break
                end
                set -l occurrences 0
                for prior in $seen
                    if test "$prior" = "$flag"
                        set occurrences (math $occurrences + 1)
                    end
                end
                if test $occurrences -gt 1; and not contains -- $flag $repeats
                    set valid 0
                end
                for peer in (__orchbee_meta "conflicts|$key|$flag")
                    if contains -- $peer $seen
                        set valid 0
                    end
                end
            end
            test $valid -eq 1; or continue
            for index in (seq (count $value_flags))
                set -l flag $value_flags[$index]
                if not __orchbee_string "$key|$flag" $values[$index]
                    set valid 0
                end
                if test (count (__orchbee_meta "closed|$key|$flag")) -gt 0
                    if not contains -- $values[$index] (__orchbee_meta "values|$key|$flag")
                        set valid 0
                    end
                end
            end
            test $valid -eq 1; or continue
            set -l limit (__orchbee_limit $key '' $seen)
            test $n -le (math $width + $limit); or continue
            if test $n -gt $width
                set -l operand_index (math $width + 1)
                __orchbee_string "$key|operand" $positionals[$operand_index]; or continue
                set -l equal (__orchbee_meta "operand-equals|$key")
                if test (count $equal) -gt 0
                    for index in (seq (count $value_flags))
                        if test "$value_flags[$index]" = "$equal"; and test "$values[$index]" != "$positionals[$operand_index]"
                            set valid 0
                        end
                    end
                    test $valid -eq 1; or continue
                end
            end
            set -a viable $key
            if test $n -lt $width
                set -l next (math $n + 1)
                set -a candidates $path[$next]
            end
            set -l selectors (__orchbee_meta "selectors|$key")
            set -l selected 0
            for flag in $selectors
                contains -- $flag $seen; and set selected 1
            end
            if test $n -eq $width; and test $selected -eq 0; and test (count (__orchbee_meta "agent|$key")) -gt 0
                set -a candidates (__orchbee_meta agent-targets)
            end
            set -l operands (math $n - $width)
            set -l offset (math (__orchbee_meta "offset|$key") - $selected)
            if test $operands -ge $offset; and test $operands -lt $limit
                set -a candidates (__orchbee_meta "operand-values|$key")
            end
            for flag in $allowed
                if test $machine -eq 1; and test "$flag" = --session
                    continue
                end
                if test $operands -gt 0; and contains -- $flag $selectors
                    continue
                end
                if contains -- $flag $seen; and not contains -- $flag $repeats
                    continue
                end
                set -l conflict 0
                for peer in (__orchbee_meta "conflicts|$key|$flag")
                    contains -- $peer $seen; and set conflict 1
                end
                test $conflict -eq 0; or continue
                set -l required (__orchbee_meta "requires|$key|$flag")
                set -l missing (count $required)
                for peer in $required
                    contains -- $peer $seen; and set missing 0
                end
                test $missing -eq 0; or continue
                test $operands -le (__orchbee_limit $key $flag $seen); or continue
                set -a offered $flag
            end
            set -a candidates --help
        end
        test (count $viable) -gt 0; or return
        set -l joined ''
        if test -z "$pending"; and string match -q -- '--*=*' $prefix
            set -l pair (string split -m 1 = -- $prefix)
            contains -- $pair[1] $offered; or return
            contains -- $pair[1] (__orchbee_meta joined-flags); or return
            set pending $pair[1]
            set joined "$pending="
        end
        if test -n "$pending"
            set candidates
            for key in $viable
                for value in (__orchbee_meta "values|$key|$pending")
                    set -a candidates "$joined$value"
                end
            end
        else
            set -a candidates $offered
            if test $n -eq 0; and test $business -eq 0
                set -a candidates help
                if test $machine -eq 0
                    set -a candidates completion
                    set -l empty_modes (__orchbee_meta empty-modes)
                    for flag in $modes
                        if test (count $seen) -gt 0; and contains -- $flag $empty_modes
                            continue
                        end
                        set -a candidates $flag
                    end
                end
            end
        end
    end
    if test $original_count -eq 0; and test $machine -eq 0
        set -a candidates (__orchbee_meta prefix)
    end
    set -l emitted
    for candidate in $candidates
        if string match -qr -- '^'(string escape --style=regex -- $prefix) $candidate
            if not contains -- $candidate $emitted
                printf '%s\n' $candidate
                set -a emitted $candidate
            end
        end
    end
end
complete -c orchbee -f -a '(__orchbee_candidates)'
