# Установка DropForge (Tauri) в пользовательскую папку и ярлык на рабочем столе.
#   powershell -ExecutionPolicy Bypass -File install.ps1
param(
  [string]$Target = "$env:LOCALAPPDATA\Programs\DropForge"
)

$ErrorActionPreference = 'Stop'
$source = Split-Path -Parent $MyInvocation.MyCommand.Path
Write-Host "Откуда: $source"
Write-Host "Куда:   $Target"

New-Item -ItemType Directory -Force -Path $Target | Out-Null

# Профиль пользователя не затираем при обновлении
$keepData = Test-Path (Join-Path $Target 'data')
if ($keepData) {
  Write-Host 'Профиль уже есть — сохраняю data/'
  $temp = Join-Path $env:TEMP ('dropforge-data-' + [guid]::NewGuid().ToString('n'))
  Copy-Item (Join-Path $Target 'data') $temp -Recurse -Force
} else {
  $temp = $null
}

Get-ChildItem $source -Force | Where-Object { $_.Name -notin @('install.ps1', 'data') } | ForEach-Object {
  Copy-Item $_.FullName $Target -Recurse -Force
}

if ($temp) {
  Remove-Item (Join-Path $Target 'data') -Recurse -Force -ErrorAction SilentlyContinue
  Copy-Item $temp (Join-Path $Target 'data') -Recurse -Force
  Remove-Item $temp -Recurse -Force -ErrorAction SilentlyContinue
}

$exe = Join-Path $Target 'DropForge.exe'
if (-not (Test-Path $exe)) {
  throw "Не найден $exe"
}

$desktop = [Environment]::GetFolderPath('Desktop')
$shortcut = Join-Path $desktop 'DropForge.lnk'
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($shortcut)
$link.TargetPath = $exe
$link.WorkingDirectory = $Target
$link.IconLocation = $exe
$link.Description = 'DropForge — автофарм Twitch Drops'
$link.Save()

Write-Host ''
Write-Host "Готово. Запуск: $exe"
Write-Host "Ярлык: $shortcut"
Write-Host 'Профиль и настройки лежат в data/ рядом с DropForge.exe.'
