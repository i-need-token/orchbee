# Static Orchbee completion. Evaluate explicitly in an interactive Elvish session.
{
  use str
  use re
  var grammar = (print '__ORCHBEE_GRAMMAR__' | from-json)
  var groups = [
    [--timeout --forever] [--agent-session --name --panel] [--panel --current]
    [--workspace --standalone] [--exec --profile] [--arg --profile]
    [--into-tab --new-tab] [--target-panel --new-tab]
    [--text --file --stdin --key] [--key --enter] [--key --paste]
    [--text --paste] [--match --regex]
  ]
  var panel-targets = [panel:process-info panel:focus panel:zoom panel:layout panel:neighbor panel:edges panel:resize panel:swap panel:input panel:split]
  var agent-targets = [agent:attach agent:get agent:focus agent:read agent:send-keys agent:prompt agent:explain agent:wait agent:resolve agent:rename]
  fn any {|values choices|
    for choice $choices { if (has-value $values $choice) { put $true; return } }
    put $false
  }
  fn conflicts {|command flag seen|
    for pair $command[conflicts] {
      if (and (eq $pair[flag] $flag) (any $seen $pair[with])) { put $true; return }
    }
    for group $groups {
      if (has-value $group $flag) {
        for peer $group {
          if (and (not-eq $peer $flag) (has-value $seen $peer)) { put $true; return }
        }
      }
    }
    var path = (str:join : $command[words])
    if (eq $command[words][0] worktree) {
      if (or (and (eq $flag --workspace) (has-value $seen --cwd)) (and (eq $flag --cwd) (has-value $seen --workspace))) { put $true; return }
    }
    if (eq $path worktree:open) {
      if (or (and (eq $flag --path) (has-value $seen --branch)) (and (eq $flag --branch) (has-value $seen --path))) { put $true; return }
    }
    if (eq $path worktree:list) {
      if (or (and (has-value [--project --directory] $flag) (any $seen [--workspace --cwd --trust-repository])) (and (has-value [--workspace --cwd --trust-repository] $flag) (any $seen [--project --directory]))) { put $true; return }
    }
    put $false
  }
  fn complete {|@args|
    if (< (count $args) 2) { return }
    var prefix = $args[-1]
    var positionals seen = [] []
    var values = [&]
    var pending = ''
    for word $args[1..-1] {
      if (eq $word --) { return }
      if (not-eq $pending '') { set values[$pending] = $word; set pending = ''; continue }
      if (has-value [--help -h --version -v -V --skill__ORCHBEE_DEFAULT_CONFIG_ELVISH__] $word) { return }
      if (and (str:has-prefix $word -) (> (count $word) 1)) {
        var pair = [(str:split &max=2 = $word)]
        var flag = $pair[0]
        if (not (has-value $grammar[flags] $flag)) { return }
        if (and (has-value $seen $flag) (not (has-value $grammar[repeatable] $flag)) (not-eq $flag --provider)) { return }
        set seen = [$@seen $flag]
        if (has-value $grammar[valueFlags] $flag) {
          if (== (count $pair) 2) { set values[$flag] = $pair[1] } else { set pending = $flag }
        } elif (== (count $pair) 2) { return }
      } else { set positionals = [$@positionals $word] }
    }
    fn valid-name {|name| put (and (re:match '^[A-Za-z0-9._-]{1,64}$' $name) (not (has-value [. ..] $name))) }
    if (and (has-key $values --session) (not (valid-name $values[--session]))) { return }
    if (and (> (count $positionals) 2) (eq $positionals[0] session) (has-value [attach stop delete] $positionals[1])) {
      if (not (valid-name $positionals[2])) { return }
      if (and (has-key $values --session) (not-eq $values[--session] $positionals[2])) { return }
    }
    var business-seen = []
    for flag $seen { if (not (has-value $grammar[globalFlags] $flag)) { set business-seen = [$@business-seen $flag] } }
    var candidates = []
    var n = (count $positionals)
    if (and (> $n 0) (has-value [completion help] $positionals[0])) {
      if (or (> (count $business-seen) 0) (not-eq $pending '')) { return }
      if (eq $positionals[0] completion) {
        if (!= $n 1) { return }
        var shells = $grammar[shells]
        set candidates = [$@shells --help]
      } elif (== $n 1) { set candidates = $grammar[helpTopics]
      } elif (== $n 2) {
        for entry $grammar[helpChildren] { if (eq $entry[0] $positionals[1]) { set candidates = $entry[1] } }
      } elif (== $n 3) {
        for entry $grammar[helpLevel3] { if (eq $entry[0] $positionals[1]':'$positionals[2]) { set candidates = $entry[1] } }
      }
    } else {
      for entry [[--format $grammar[formats]] [--mouse [off on]] [--until $grammar[until]] [--right-click $grammar[rightClick]]] {
        if (and (has-key $values $entry[0]) (not (has-value $entry[1] $values[$entry[0]]))) { return }
      }
      var viable = []
      for command $grammar[commands] {
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
        var path = (str:join : $command[words])
        if (has-key $values --source) {
          if (eq $path workspace:report-metadata) {
            if (not (re:match '^[A-Za-z0-9:._-]{1,80}$' (str:trim-space $values[--source]))) { set valid = $false }
          } elif (not (has-value $grammar[outputSources] $values[--source])) { set valid = $false }
        }

        var provider-count = 0
        for flag $seen {
          if (or (not (has-value $allowed $flag)) (conflicts $command $flag $seen)) { set valid = $false }
          if (eq $flag --provider) { set provider-count = (+ $provider-count 1) }
        }
        if (and (> $provider-count 1) (not-eq $path agent:doctor)) { set valid = $false }
        var limit = $command[operand]
        if (and (eq $path panel:process-info) (has-value $seen --client)) { set limit = 0 }
        if (and (has-value $panel-targets $path) (any $seen [--panel --current])) { set limit = 0 }
        if (has-value $agent-targets $path) {
          if (any $seen [--agent-session --name --panel]) { set limit = (- $limit 1) }
          if (has-value $seen --clear) { set limit = (- $limit 1) }
        }
        if (> $n (+ $width $limit)) { set valid = $false }
        if (and (has-key $values --motion) (not (has-value $grammar[copyMotions] $values[--motion]))) { set valid = $false }
        if (and (has-key $values --direction) (not (has-value $command[directions] $values[--direction]))) { set valid = $false }
        if $valid { set viable = [$@viable $command] }
      }
      if (== (count $viable) 0) { return }
      var offered = []
      for command $viable {
        var width = (count $command[words])
        var path = (str:join : $command[words])
        if (< $n $width) { set candidates = [$@candidates $command[words][$n]] }
        if (eq $path agent:send-keys) {
          var target = 1
          if (any $seen [--agent-session --name --panel]) { set target = 0 }
          var keys = (- $n $width $target)
          if (and (>= $keys 0) (< $keys 64)) { var names = $grammar[keys]; set candidates = [$@candidates $@names] }
        }
        var global = $command[globalFlags]
        var flags = $command[flags]
        var allowed = [$@flags $@global]
        if $command[format] { set allowed = [$@allowed --format] }
        for flag $allowed {
          if (and (eq $path panel:process-info) (> $n $width) (eq $flag --client)) { continue }
          if (and (has-value $panel-targets $path) (> $n $width) (has-value [--panel --current] $flag)) { continue }
          if (has-value $agent-targets $path) {
            var operands = (- $n $width)
            if (and (> $operands 0) (has-value [--name --agent-session --panel] $flag)) { continue }
            if (and (eq $flag --clear) (or (> $operands 1) (and (> $operands 0) (any $seen [--name --agent-session --panel])))) { continue }
          }
          if (and (has-value $seen $flag) (not (has-value $grammar[repeatable] $flag)) (not (and (eq $flag --provider) (eq $path agent:doctor)))) { continue }
          if (and (eq $flag --arg) (not (has-value $seen --exec)) (not-eq $path agent:start)) { continue }
          if (not (conflicts $command $flag $seen)) { set offered = [$@offered $flag] }
        }
        set candidates = [$@candidates --help]
      }
      var joined = ''
      if (and (eq $pending '') (str:has-prefix $prefix --) (str:contains $prefix =)) {
        var flag = [(str:split &max=2 = $prefix)][0]
        if (not (has-value $offered $flag)) { return }
        set pending = $flag
        set joined = $flag'='
      }
      if (not-eq $pending '') {
        set candidates = []
        var source-values = []
        for command $viable { if (not-eq (str:join : $command[words]) workspace:report-metadata) { set source-values = $grammar[outputSources]; break } }
        var enums = [&--motion=$grammar[copyMotions] &--format=$grammar[formats] &--mouse=[off on] &--right-click=$grammar[rightClick] &--source=$source-values &--key=$grammar[keys] &--until=$grammar[until] &--provider=$grammar[providers] &--kind=$grammar[agentKinds]]
        if (has-key $enums $pending) {
          for value $enums[$pending] { set candidates = [$@candidates $joined$value] }
        } elif (eq $pending --cwd) {
          for command $viable { for value $command[cwdValues] { set candidates = [$@candidates $joined$value] } }
        } elif (eq $pending --direction) {
          for command $viable { for value $command[directions] { set candidates = [$@candidates $joined$value] } }
        }
      } else {
        set candidates = [$@candidates $@offered]
        if (and (== $n 0) (== (count $seen) 0)) { set candidates = [$@candidates help completion --version -v -V --skill__ORCHBEE_DEFAULT_CONFIG_ELVISH__] }
      }
    }
    # The editor owns prefix/fuzzy matching and quoting. Emit each static value once.
    var emitted = [&]
    for candidate $candidates {
      if (not (has-key $emitted $candidate)) { put $candidate; set emitted[$candidate] = $true }
    }
  }
  set edit:completion:arg-completer[orchbee] = $complete~
  set edit:completion:arg-completer[orchbee.exe] = $complete~
}
