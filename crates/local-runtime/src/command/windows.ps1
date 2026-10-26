function q([string]$v, [bool]$batch) {
    $out = [string][char]34
    $slashes = 0
    foreach ($ch in $v.ToCharArray()) {
        if ($ch -eq [char]92) { $slashes++; continue }
        $count = if ($ch -eq [char]34) { 2 * $slashes } else { $slashes }
        $out += ('\' * $count)
        $slashes = 0
        if ($ch -eq [char]34) {
            $out += $(if ($batch) { [char]34 } else { [char]92 })
        }
        if ($batch -and $ch -eq '%') { $out += '%%cd:~,' }
        $out += $ch
    }
    $out += ('\' * (2 * $slashes)) + [char]34
    return $out
}
if ($c.CommandType -eq 'ExternalScript') {
    & $c.Path @a
} elseif ([IO.Path]::GetExtension($c.Path) -in '.cmd', '.bat') {
    $joined = @($a | ForEach-Object { q $_ $true }) -join ' '
    $quote = [string][char]34
    $line = '/e:ON /v:OFF /d /c ' + $quote + $quote + $c.Path.Replace('%', '%%cd:~,%') + $quote + ' ' + $joined + $quote
    if ($line.Length -gt 8000) { throw 'Windows batch command exceeds 8000 characters' }
    Start-Process -FilePath ($env:SystemRoot + '\System32\cmd.exe') -ArgumentList $line -NoNewWindow -Wait
} elseif ($a.Count -eq 0) {
    Start-Process -FilePath $c.Path -NoNewWindow -Wait
} else {
    $joined = @($a | ForEach-Object { q $_ $false }) -join ' '
    Start-Process -FilePath $c.Path -ArgumentList $joined -NoNewWindow -Wait
}
