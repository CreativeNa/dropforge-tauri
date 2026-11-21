# Создаёт ярлык DropForge на рабочем столе и в меню «Пуск».
# Запуск: npm run shortcut
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'dist\DropForge-win32-x64\DropForge.exe'

if (-not (Test-Path $exe)) {
    Write-Error "Сначала соберите приложение: npm run build`nНе найден: $exe"
    exit 1
}

$shell = New-Object -ComObject WScript.Shell
$targets = @(
    (Join-Path ([Environment]::GetFolderPath('Desktop')) 'DropForge.lnk'),
    (Join-Path ([Environment]::GetFolderPath('Programs')) 'DropForge.lnk')
)

foreach ($path in $targets) {
    $shortcut = $shell.CreateShortcut($path)
    $shortcut.TargetPath = $exe
    $shortcut.WorkingDirectory = Split-Path $exe
    $shortcut.IconLocation = "$exe,0"
    $shortcut.Description = 'DropForge — автофарм Twitch Drops'
    $shortcut.WindowStyle = 1
    $shortcut.Save()
    Write-Host "Ярлык создан: $path"
}
