# Fills the native folder picker owned by a given process without keyboard focus:
# WM_SETTEXT puts the path in the "Folder:" edit (id 1152), BM_CLICK presses
# "Select Folder" (id 1). -Close cancels it (WM_CLOSE). No screenshot of the dialog is
# saved (it shows account and drive names). Local test tool.
param([int]$OwnerPid, [string]$Folder, [switch]$Close)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type -Namespace W3 -Name U -MemberDefinition @'
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern System.IntPtr SendMessage(System.IntPtr h, uint m, System.IntPtr w, string l);
[DllImport("user32.dll")] public static extern System.IntPtr SendMessage(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);
'@
$A = [System.Windows.Automation.AutomationElement]
$cond = [System.Windows.Automation.PropertyCondition]::new($A::ProcessIdProperty, $OwnerPid)
$dlg = $null
for ($i = 0; $i -lt 60 -and -not $dlg; $i++) {
  $dlg = $A::RootElement.FindAll('Children', $cond) | Where-Object { $_.Current.ClassName -eq '#32770' } | Select-Object -First 1
  if (-not $dlg) { Start-Sleep -Milliseconds 250 }
}
if (-not $dlg) { 'NO DIALOG'; exit 1 }
if ($Close) { [W3.U]::SendMessage([IntPtr]$dlg.Current.NativeWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null; 'closed'; exit 0 }
"dialog: $($dlg.Current.Name)"
$w = [System.Windows.Automation.TreeWalker]::RawViewWalker
$c = $w.GetFirstChild($dlg); $edit = $null; $ok = $null
while ($c) {
  if ($c.Current.AutomationId -eq '1152') { $edit = $c }
  if ($c.Current.AutomationId -eq '1' -and $c.Current.ClassName -eq 'Button') { $ok = $c }
  $c = $w.GetNextSibling($c)
}
if (-not $edit -or -not $ok) { 'CONTROLS NOT FOUND'; exit 1 }
[W3.U]::SendMessage([IntPtr]$edit.Current.NativeWindowHandle, 0x000C, [IntPtr]::Zero, $Folder) | Out-Null
Start-Sleep -Milliseconds 300
[W3.U]::SendMessage([IntPtr]$ok.Current.NativeWindowHandle, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 1000
$still = $A::RootElement.FindAll('Children', $cond) | Where-Object { $_.Current.ClassName -eq '#32770' }
if ($still) { 'DIALOG STILL OPEN' } else { 'dialog closed' }
