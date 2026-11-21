# DropForge 1.3.14 (Tauri)

## Исправлена установка: сервисы не попадали в установленную копию

Установщик NSIS раскладывал `watcher-svc` и `pubsub-svc` во вложенную папку `_up_\...`, из-за чего в свежей установке:

- `watcher-svc\dropforge-watcher-svc.exe` и `pubsub-svc\dropforge-pubsub-svc.exe` отсутствовали;
- фарм не запускался («Автофарм остановлен» сразу после старта);
- диагностика показывала `watcher-svc — ОТСУТСТВУЕТ`, `status: null`, `progress.drops: 0`.

Что сделано:

- ресурсы установщика теперь кладутся прямо в `watcher-svc\` и `pubsub-svc\`;
- приложение дополнительно ищет сервисы в подпапках (на случай старых установок);
- при старте установленной копии удаляются остатки `_up_\` и дубликат `DropForge.exe`.

## Проверка после установки

```
%LOCALAPPDATA%\DropForge\watcher-svc\dropforge-watcher-svc.exe
%LOCALAPPDATA%\DropForge\pubsub-svc\dropforge-pubsub-svc.exe
```

Оба файла должны быть на месте, а в диагностике — `watcher pid:` и заполненный `status`.

## Напоминание

Если приложение запускается из старой портативной папки (например, `Downloads\DropForge-1.3.x-win64`), появится баннер «Запущена устаревшая копия приложения» с кнопкой перехода на установленную версию.

## Проверка архивов

```
SHA256  6E0F93D8367581C1C50BBF3A73290B7732854C0E95AC56ADFEDF0651A30663D9  DropForge-1.3.14-win64.zip

SHA256  F93B88B81C601F4489BC4118DDA6CC9536761D46B89EE1D01EF44400C2C0FF24  DropForge_1.3.14_x64-setup.exe
```
