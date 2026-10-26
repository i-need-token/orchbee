# Static Orchbee completion. Evaluate explicitly in an interactive Elvish session.
{
  use str
  use re
  var grammar = (print '__ORCHBEE_GRAMMAR__' | from-json)
  fn any {|values choices|
    for choice $choices { if (has-value $values $choice) { put $true; return } }
    put $false
  }
  fn conflicts {|command flag seen|
    for pair $command[conflicts] {
      if (and (eq $pair[flag] $flag) (any $seen $pair[with])) { put $true; return }
    }
    put $false
  }
  fn complete {|@args|
    if (< (count $args) 2) { return }
    var prefix = $args[-1]
    var machine = $false
    var words = $args[1..-1]
    if (> (count $words) 0) {
      if (eq $words[0] --machine) {
        if (< (count $words) 2) { return }
        if (or (eq $words[1] '') (str:has-prefix $words[1] -)) { return }
        set machine = $true
        set words = $words[2..]
      } elif (str:has-prefix $words[0] --machine=) {
        var selector = [(str:split &max=2 = $words[0])][1]
        if (or (eq $selector '') (str:has-prefix $selector -)) { return }
        set machine = $true
        set words = $words[1..]
      }
    }
    var positionals seen = [] []
    var values = [&]
    var pending = ''
    for word $words {
      if (eq $word --) { return }
      if (not-eq $pending '') { set values[$pending] = $word; set pending = ''; continue }
      if (or (has-value $grammar[rootModeFlags] $word) (eq $word -h)) { return }
      if (and (str:has-prefix $word -) (> (count $word) 1)) {
        var pair = [(str:split &max=2 = $word)]
        var flag = $pair[0]
        if (not (has-value $grammar[flags] $flag)) { return }
        if (and (has-value $seen $flag) (not (has-value $grammar[repeatable] $flag))) { return }
        if (and (== (count $pair) 2) (not (has-value $grammar[joinedFlags] $flag))) { return }
        set seen = [$@seen $flag]
        if (has-value $grammar[valueFlags] $flag) {
          if (== (count $pair) 2) { set values[$flag] = $pair[1] } else { set pending = $flag }
        } elif (== (count $pair) 2) { return }
      } else { set positionals = [$@positionals $word] }
    }
    fn valid-string {|value rule|
      if (eq $rule $nil) { put $true; return }
      if $rule[trim] { set value = (str:trim-space $value) }
      put (and (re:match $rule[pattern] $value) (not (has-value $rule[excluded] $value)))
    }
    if (and $machine (has-value $seen --session)) { return }
    var business-seen = []
    for flag $seen { if (not (has-value $grammar[globalFlags] $flag)) { set business-seen = [$@business-seen $flag] } }
    var candidates = []
    var n = (count $positionals)
    if (and (> $n 0) (has-value [completion help] $positionals[0])) {
      if (or (> (count $business-seen) 0) (not-eq $pending '')) { return }
      if (eq $positionals[0] completion) {
        if (or (!= $n 1) $machine) { return }
        var shells = $grammar[shells]
        set candidates = [$@shells --help]
      } elif (== $n 1) { set candidates = $grammar[helpTopics]
      } elif (== $n 2) {
        for entry $grammar[helpChildren] { if (eq $entry[0] $positionals[1]) { set candidates = $entry[1] } }
      } elif (== $n 3) {
        for entry $grammar[helpLevel3] { if (eq $entry[0] $positionals[1]':'$positionals[2]) { set candidates = $entry[1] } }
      }
    } else {
      var viable = []
      for command $grammar[commands] {
        if (and $machine (not $command[machine])) { continue }
        # Reject unrelated paths before walking their option/conflict tables.
        var width = (count $command[words])
        var valid = $true
        for index [(range $width)] {
          if (and (< $index $n) (not-eq $positionals[$index] $command[words][$index])) { set valid = $false; break }
        }
        if (not $valid) { continue }
        var global = $command[globalFlags]
        var flags = $command[flags]
        var allowed = [$@flags $@global]
        if $command[format] { set allowed = [$@allowed --format] }
        for flag $seen {
          if (or (not (has-value $allowed $flag)) (conflicts $command $flag $seen)) { set valid = $false }
          var occurrences = 0
          for prior $seen { if (eq $prior $flag) { set occurrences = (+ $occurrences 1) } }
          if (and (> $occurrences 1) (not (has-value $command[repeatable] $flag))) { set valid = $false }
        }
        for value $command[values] {
          if (and $value[closed] (has-key $values $value[flag]) (not (has-value $value[choices] $values[$value[flag]]))) { set valid = $false }
          if (and (has-key $values $value[flag]) (not (valid-string $values[$value[flag]] $value[string]))) { set valid = $false }
        }
        var limit = $command[operand]
        for group $command[reductions] { if (any $seen $group) { set limit = (- $limit 1) } }
        if (> $n (+ $width $limit)) { set valid = $false }
        if (and (> $n $width) (not-eq $command[operandString] $nil)) {
          if (not (valid-string $positionals[$width] $command[operandString])) { set valid = $false }
          if (and (has-key $values $command[operandEquals]) (not-eq $values[$command[operandEquals]] $positionals[$width])) { set valid = $false }
        }
        if $valid { set viable = [$@viable $command] }
      }
      if (== (count $viable) 0) { return }
      var offered = []
      for command $viable {
        var width = (count $command[words])
        if (< $n $width) { set candidates = [$@candidates $command[words][$n]] }
        var selected = (any $seen $command[selectors])
        if (and $command[agentTarget] (== $n $width) (not $selected)) { set candidates = [$@candidates $@grammar[agentTargets]] }
        var offset = $command[operandValueOffset]
        if $selected { set offset = (- $offset 1) }
        var operands = (- $n $width)
        var limit = $command[operand]
        for group $command[reductions] { if (any $seen $group) { set limit = (- $limit 1) } }
        if (and (>= $operands $offset) (< $operands $limit)) { set candidates = [$@candidates $@command[operandValues]] }
        var global = $command[globalFlags]
        var flags = $command[flags]
        var allowed = [$@flags $@global]
        if $command[format] { set allowed = [$@allowed --format] }
        for flag $allowed {
          if (and $machine (eq $flag --session)) { continue }
          if (and (> $operands 0) (has-value $command[selectors] $flag)) { continue }
          if (and (has-value $seen $flag) (not (has-value $command[repeatable] $flag))) { continue }
          var proposed = $command[operand]
          for group $command[reductions] { if (or (has-value $group $flag) (any $seen $group)) { set proposed = (- $proposed 1) } }
          if (> $operands $proposed) { continue }
          var missing = $false
          for rule $command[requirements] { if (and (eq $rule[flag] $flag) (not (any $seen $rule[any]))) { set missing = $true } }
          if $missing { continue }
          if (not (conflicts $command $flag $seen)) { set offered = [$@offered $flag] }
        }
        set candidates = [$@candidates --help]
      }
      var joined = ''
      if (and (eq $pending '') (str:has-prefix $prefix --) (str:contains $prefix =)) {
        var flag = [(str:split &max=2 = $prefix)][0]
        if (not (has-value $offered $flag)) { return }
        if (not (has-value $grammar[joinedFlags] $flag)) { return }
        set pending = $flag
        set joined = $flag'='
      }
      if (not-eq $pending '') {
        set candidates = []
        for command $viable {
          for entry $command[values] {
            if (eq $entry[flag] $pending) {
              for value $entry[choices] { set candidates = [$@candidates $joined$value] }
            }
          }
        }
      } else {
        set candidates = [$@candidates $@offered]
        if (and (== $n 0) (== (count $business-seen) 0)) {
          set candidates = [$@candidates help]
          if (not $machine) {
            set candidates = [$@candidates completion]
            for flag $grammar[rootModeFlags] {
              if (and (> (count $seen) 0) (has-value $grammar[rootModeFlagsRequireNoSeenFlags] $flag)) { continue }
              set candidates = [$@candidates $flag]
            }
          }
        }
      }
    }
    if (and (== (count $words) 0) (not $machine)) { set candidates = [$@candidates $@grammar[prefixFlags]] }
    # The editor owns prefix/fuzzy matching and quoting. Emit each static value once.
    var emitted = [&]
    for candidate $candidates {
      if (not (has-key $emitted $candidate)) { put $candidate; set emitted[$candidate] = $true }
    }
  }
  set edit:completion:arg-completer[orchbee] = $complete~
  set edit:completion:arg-completer[orchbee.exe] = $complete~
}
