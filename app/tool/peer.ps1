# The other device for integration_test/with_friend_test.dart on Windows, like peer.sh: a
# command-line Fogata called Marta that shows a friend link, accepts the request, waits to
# be added to a group, then enters its room, says hello, shares a file and leaves.
#
#   powershell -ep bypass -f tool\peer.ps1 <fogata.exe> <empty data folder> <output folder>
param(
  [Parameter(Mandatory)] [string] $Fogata,
  [Parameter(Mandatory)] [string] $Data,
  [Parameter(Mandatory)] [string] $Out
)
New-Item -ItemType Directory -Force $Out | Out-Null
$bytes = New-Object byte[] 3000000
(New-Object Random).NextBytes($bytes)
[IO.File]::WriteAllBytes("$Out\test-photo.bin", $bytes)

# Starts the CLI with its output going to files as it is written, and its input open.
function Start-Fogata([string] $arguments, [string] $name) {
  $info = New-Object Diagnostics.ProcessStartInfo $Fogata, "--data-dir `"$Data`" $arguments"
  $info.UseShellExecute = $false
  $info.RedirectStandardInput = $true
  $info.RedirectStandardOutput = $true
  $info.RedirectStandardError = $true
  $process = [Diagnostics.Process]::Start($info)
  $files = @()
  foreach ($pair in @(@($process.StandardOutput.BaseStream, "$Out\$name.out"),
                      @($process.StandardError.BaseStream, "$Out\$name.err"))) {
    $file = New-Object IO.FileStream $pair[1], 'Create', 'Write', 'ReadWrite', 1
    $null = $pair[0].CopyToAsync($file, 64)
    $files += $file
  }
  [pscustomobject]@{ Process = $process; Files = $files }
}

function Say($fogata, [string] $line) {
  try { $fogata.Process.StandardInput.WriteLine($line); $fogata.Process.StandardInput.Flush() } catch {}
}

function Finish($fogata) {
  if (-not $fogata.Process.HasExited) { $fogata.Process.Kill() }
  $fogata.Process.WaitForExit()
  Start-Sleep -Milliseconds 300
  $fogata.Files | ForEach-Object { $_.Close() }
}

& $Fogata --data-dir $Data profile --name Marta *> "$Out\profile.out"

# Answers "y" to the request when it comes.
$invite = Start-Fogata 'friends invite' 'invite'
while (-not $invite.Process.HasExited) { Start-Sleep 2; Say $invite 'y' }
Finish $invite

$online = Start-Fogata 'friends online' 'online'
for ($i = 0; $i -lt 100; $i++) {
  if ((& $Fogata --data-dir $Data group list 2>$null) -match 'The usual crew') { break }
  Start-Sleep 3
}
Finish $online
Start-Sleep 2
# The group arrived as an invitation from a friend: join it.
& $Fogata --data-dir $Data group accept "The usual crew" *> "$Out\accept.out"
# The group reaches Marta as soon as it is made: she waits so that the app is in the room
# before her and plays the sound of her coming in.
Start-Sleep 10

$chat = Start-Fogata "chat `"The usual crew`" --downloads `"$Out`"" 'chat'
Start-Sleep 20
Say $chat 'hi from Marta'
Start-Sleep 2
Say $chat "/share $Out\test-photo.bin"
Start-Sleep 60
Say $chat 'heading off now'
Start-Sleep 3
Say $chat '/quit'
$chat.Process.WaitForExit(30000) | Out-Null
Finish $chat
'done' | Out-File "$Out\finished" -Encoding ascii
