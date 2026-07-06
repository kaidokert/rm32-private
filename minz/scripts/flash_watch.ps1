# Flash watcher for the flaky-SWD bench: retry erase+download+reset
# until the ST-LINK answers (e.g. after a USB replug / board powercycle).
# Usage: powershell -File scripts\flash_watch.ps1 [-Elf <path>] [-Tries 110]
param(
    [string]$Probe = '0483:374f:0037002F3234510836303532',
    [string]$Chip = 'STM32L431KCUx',
    [string]$Elf = 'E:\m\robot\esc\rm32\minz\target\thumbv7em-none-eabihf\release\examples\motor_tester2',
    [int]$Tries = 110,
    [int]$DelaySec = 8
)

for ($i = 0; $i -lt $Tries; $i++) {
    $e = probe-rs erase --chip $Chip --probe $Probe 2>&1 | Out-String
    if ($LASTEXITCODE -eq 0) {
        Write-Output "try $i : ERASE OK"
        $d = probe-rs download --chip $Chip --probe $Probe $Elf 2>&1 | Out-String
        if ($LASTEXITCODE -eq 0) {
            Write-Output 'DOWNLOAD OK'
            probe-rs reset --chip $Chip --probe $Probe 2>&1 | Out-Null
            Write-Output 'FLASH COMPLETE'
            exit 0
        }
        $last = ($d.Trim() -split '\r?\n') | Select-Object -Last 1
        Write-Output "download failed: $last"
    } elseif ($i % 8 -eq 0) {
        $last = ($e.Trim() -split '\r?\n') | Select-Object -Last 1
        Write-Output "try $i : $last"
    }
    Start-Sleep -Seconds $DelaySec
}
Write-Output 'WATCHER GAVE UP'
exit 1
