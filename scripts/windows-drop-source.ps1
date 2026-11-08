param([Parameter(Mandatory=$true)][string]$Config)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class DpdfDropFocus {
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr window);
}
'@
$settings = Get-Content -LiteralPath $Config -Raw -Encoding UTF8 | ConvertFrom-Json
function Write-Result([string]$Name, $Value) {
    $path = Join-Path $settings.Output $Name
    [IO.File]::WriteAllText($path + '.new', ($Value | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))
    [IO.File]::Move($path + '.new', $path)
}
$form = [Windows.Forms.Form]::new()
$form.Text = 'dPDF generated OLE source'
$form.StartPosition = 'Manual'
$form.Location = [Drawing.Point]::new([int]$settings.X, [int]$settings.Y)
$form.Size = [Drawing.Size]::new(280, 130)
$form.TopMost = $true
$label = [Windows.Forms.Label]::new()
$label.Text = 'Drag generated PDF fixtures'
$label.TextAlign = 'MiddleCenter'
$label.Dock = 'Fill'
$form.Controls.Add($label)
$paths = [Collections.Specialized.StringCollection]::new()
foreach ($path in $settings.Paths) { [void]$paths.Add([string]$path) }
$data = [Windows.Forms.DataObject]::new()
$data.SetFileDropList($paths)
$script:started = $false
$script:hovered = $false
$script:hoverAt = 0L
$watch = [Diagnostics.Stopwatch]::new()
$form.Add_Shown({
    Write-Result 'ready.json' @{ Window=$form.Handle.ToInt64(); Control=$label.Handle.ToInt64(); Pid=$PID; Count=$data.GetFileDropList().Count }
})
$label.Add_GiveFeedback({
    param($sender, $event)
    if (!$script:hovered -and (($event.Effect -band [Windows.Forms.DragDropEffects]::Copy) -ne 0)) {
        $script:hovered = $true
        $script:hoverAt = $watch.ElapsedMilliseconds
        Write-Result 'hover.json' @{ Accepted=$true }
    }
})
$label.Add_QueryContinueDrag({
    param($sender, $event)
    if (($settings.Cancel -and $script:hovered -and ($watch.ElapsedMilliseconds - $script:hoverAt) -ge 300) -or $watch.ElapsedMilliseconds -gt 10000) {
        $event.Action = [Windows.Forms.DragAction]::Cancel
    }
})
$label.Add_MouseDown({
    param($sender, $event)
    if ($event.Button -ne [Windows.Forms.MouseButtons]::Left -or $script:started) { return }
    $script:started = $true
    $watch.Start()
    try {
        $effect = $label.DoDragDrop($data, [Windows.Forms.DragDropEffects]::Copy)
        $focused = [DpdfDropFocus]::SetForegroundWindow([IntPtr]::new([long]$settings.Target))
        Write-Result 'result.json' @{ Effect=[int]$effect; Hovered=$script:hovered; Focused=$focused }
    } catch {
        Write-Result 'result.json' @{ Error=$_.Exception.Message }
    } finally { $form.Close() }
})
try { [void]$form.ShowDialog() } finally { $form.Dispose() }
