# UI Automation for a modern print dialog with a proven fixture-window owner.
# Never use foreground-window heuristics, global keys, or arbitrary Explorer UI.
param(
    [Parameter(Mandatory=$true)][long]$WindowHandle,
    [Parameter(Mandatory=$true)][int]$FixtureProcessId,
    [Parameter(Mandatory=$true)][ValidateSet('print','cancel')][string]$Action
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class DpdfDialogOwner {
    [DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr window, uint command);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    public static bool IsFixtureOwned(IntPtr window, uint expected) {
        var seen = new HashSet<IntPtr>();
        for (int depth = 0; window != IntPtr.Zero && depth < 16; depth++) {
            if (!seen.Add(window)) return false;
            uint process;
            GetWindowThreadProcessId(window, out process);
            if (process == expected) return true;
            window = GetWindow(window, 4); // GW_OWNER, not Z-order
        }
        return false;
    }
}
'@
$window = [IntPtr]$WindowHandle
if (![DpdfDialogOwner]::IsWindowVisible($window) -or
    ![DpdfDialogOwner]::IsFixtureOwned($window, [uint32]$FixtureProcessId)) {
    throw 'Print dialog is not visibly associated with this fixture process.'
}
$root = [Windows.Automation.AutomationElement]::FromHandle($window)
$condition = New-Object Windows.Automation.PropertyCondition(
    [Windows.Automation.AutomationElement]::ControlTypeProperty,
    [Windows.Automation.ControlType]::Button)
$buttons = $root.FindAll([Windows.Automation.TreeScope]::Descendants, $condition)
$ids = if ($Action -eq 'cancel') { @('CancelButton','Cancel','PrintDialogCancelButton') }
       else { @('PrintButton','Print','PrintDialogPrintButton') }
$label = if ($Action -eq 'cancel') { 'Cancel' } else { 'Print' }
$candidates = @()
$diagnostic = @()
foreach ($button in $buttons) {
    $current = $button.Current
    $diagnostic += @{ id = $current.AutomationId; name = $current.Name; enabled = $current.IsEnabled }
    if ($current.IsEnabled -and $ids -contains $current.AutomationId) { $candidates += $button }
}
# Label fallback is limited to the reported English Windows UI; unknown/localized
# controls fail with fixture-only diagnostics instead of guessing another action.
if ($candidates.Count -eq 0) {
    foreach ($button in $buttons) {
        if ($button.Current.IsEnabled -and $button.Current.Name.Replace('&','').Trim() -eq $label) {
            $candidates += $button
        }
    }
}
if ($candidates.Count -ne 1) {
    Write-Output (ConvertTo-Json -InputObject $diagnostic -Compress)
    exit 2
}
if (![DpdfDialogOwner]::IsFixtureOwned($window, [uint32]$FixtureProcessId)) {
    throw 'Fixture ownership changed before invocation.'
}
$pattern = $candidates[0].GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)
$pattern.Invoke()
Write-Output 'Invoked the uniquely identified fixture print control.'
