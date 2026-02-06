$ErrorActionPreference = "Stop"

function Test-IsDocumented([string[]] $lines, [int] $index) {
  $i = $index - 1
  while ($i -ge 0) {
    $line = $lines[$i].Trim()
    if ($line.Length -eq 0) { $i--; continue }
    if ($line.StartsWith("#[")) { $i--; continue }
    if ($line.StartsWith("///")) { return $true }
    if ($line.StartsWith("/**")) { return $true }
    return $false
  }
  return $false
}

function Get-DocCoverage([string] $root) {
  $files = Get-ChildItem -Path (Join-Path $root "crates") -Recurse -Filter "*.rs" |
    Where-Object { $_.FullName -notmatch "\\target\\" }

  $items = New-Object System.Collections.Generic.List[object]

  $reFn     = [regex]'^\s*(pub(\([^)]+\))?\s+)?(async\s+)?fn\s+[A-Za-z_]\w*'
  $reStruct = [regex]'^\s*(pub(\([^)]+\))?\s+)?struct\s+[A-Za-z_]\w*'
  $reEnum   = [regex]'^\s*(pub(\([^)]+\))?\s+)?enum\s+[A-Za-z_]\w*'
  $reTrait  = [regex]'^\s*(pub(\([^)]+\))?\s+)?trait\s+[A-Za-z_]\w*'
  $reType   = [regex]'^\s*(pub(\([^)]+\))?\s+)?type\s+[A-Za-z_]\w*'
  $reConst  = [regex]'^\s*(pub(\([^)]+\))?\s+)?const\s+[A-Za-z_]\w*'
  $reStatic = [regex]'^\s*(pub(\([^)]+\))?\s+)?static\s+[A-Za-z_]\w*'

  foreach ($f in $files) {
    $lines = Get-Content -LiteralPath $f.FullName
    for ($idx = 0; $idx -lt $lines.Length; $idx++) {
      $line = $lines[$idx]
      $kind = $null
      if ($reFn.IsMatch($line)) { $kind = "fn" }
      elseif ($reStruct.IsMatch($line)) { $kind = "struct" }
      elseif ($reEnum.IsMatch($line)) { $kind = "enum" }
      elseif ($reTrait.IsMatch($line)) { $kind = "trait" }
      elseif ($reType.IsMatch($line)) { $kind = "type" }
      elseif ($reConst.IsMatch($line)) { $kind = "const" }
      elseif ($reStatic.IsMatch($line)) { $kind = "static" }
      if ($null -ne $kind) {
        $documented = Test-IsDocumented $lines $idx
        $items.Add([pscustomobject]@{
          File = $f.FullName
          Line = ($idx + 1)
          Kind = $kind
          Documented = $documented
          Code = $line.Trim()
        }) | Out-Null
      }
    }
  }

  $total = $items.Count
  $docd = ($items | Where-Object { $_.Documented }).Count
  $pct = if ($total -eq 0) { 100.0 } else { [math]::Round(($docd * 100.0) / $total, 2) }

  return [pscustomobject]@{
    Total = $total
    Documented = $docd
    Percent = $pct
    Items = $items
  }
}

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$r = Get-DocCoverage $root

$now = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
$reportPath = Join-Path $root "docs\doc-coverage-report.md"

$undoc = $r.Items | Where-Object { -not $_.Documented } | Select-Object -First 50

$md = @()
$md += "# Doc Comment Coverage Report"
$md += ""
$md += "- GeneratedAt: $now"
$md += "- Metric: rustdoc-style doc comments (see docs/commenting-guidelines.md)"
$md += "- Coverage: $($r.Documented)/$($r.Total) = $($r.Percent)%"
$md += ""
$md += "## Undocumented Items (Top 50)"
$md += ""
if ($undoc.Count -eq 0) {
  $md += "All counted items are documented."
} else {
  foreach ($u in $undoc) {
    $md += "- $($u.Kind) $($u.File):$($u.Line)  $($u.Code)"
  }
}
$md += ""

$mdText = ($md -join "`n")
Set-Content -LiteralPath $reportPath -Value $mdText -Encoding UTF8

Write-Host "Doc comment coverage: $($r.Documented)/$($r.Total) = $($r.Percent)%"
Write-Host "Report written: $reportPath"
