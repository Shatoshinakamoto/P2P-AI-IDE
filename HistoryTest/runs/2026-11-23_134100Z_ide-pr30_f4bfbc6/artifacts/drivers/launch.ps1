# Starts an app exe with WebView2 remote debugging and an isolated app-data folder,
# set for that one process tree only (cmd.exe wrapper redirects stdout/stderr to a log).
# Prints the wrapper PID. Local test tool.
param([string]$Exe, [int]$Port, [string]$DataRoot, [string]$Log, [string]$EnvJson = '{}')
New-Item -ItemType Directory -Force "$DataRoot\Roaming", "$DataRoot\Local" | Out-Null
$psi = [System.Diagnostics.ProcessStartInfo]::new("$env:SystemRoot\System32\cmd.exe")
$psi.Arguments = "/d /c `"`"$Exe`" > `"$Log`" 2>&1`""
$psi.UseShellExecute = $false
$psi.CreateNoWindow = $true
$psi.WorkingDirectory = Split-Path $Exe
$psi.Environment['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = "--remote-debugging-port=$Port"
$psi.Environment['APPDATA'] = "$DataRoot\Roaming"
$psi.Environment['LOCALAPPDATA'] = "$DataRoot\Local"
($EnvJson | ConvertFrom-Json).PSObject.Properties | ForEach-Object { $psi.Environment[$_.Name] = [string]$_.Value }
$p = [System.Diagnostics.Process]::Start($psi)
$p.Id
