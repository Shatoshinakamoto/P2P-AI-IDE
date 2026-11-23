# Moves the main window of a process (by exe name, started after a given time) off-screen so it
# never shows on the owner's desktop while it keeps rendering for CDP screenshots. Local test tool.
param([string]$Name, [int]$TimeoutS = 60)
Add-Type -Namespace W4 -Name U -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetWindowPos(System.IntPtr h, System.IntPtr a, int x, int y, int cx, int cy, uint f);
'@
for ($i = 0; $i -lt $TimeoutS * 4; $i++) {
  $p = Get-Process -Name $Name -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  if ($p) {
    # SWP_NOZORDER|SWP_NOACTIVATE: keep size 1280x800, move to -32000,-32000, do not take focus.
    [W4.U]::SetWindowPos($p.MainWindowHandle, [IntPtr]::Zero, -32000, -32000, 1280, 800, 0x0014) | Out-Null
    "moved $($p.Id)"; exit 0
  }
  Start-Sleep -Milliseconds 250
}
'NO WINDOW'; exit 1
