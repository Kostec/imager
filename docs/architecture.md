# Архитектура

Wukong Imager — форк `armbian/imager` (Tauri 2: Rust-бэкенд, React/TypeScript-фронтенд). Он записывает образ на SD-карту Wukong Pi Zero и заранее кладёт на неё настройки первого включения. Словарь — `CONTEXT.md`; почему решения такие — `docs/adr/`. Здесь только устройство и правила.

## Поток данных

Прошивка с настройками:

1. Фронтенд берёт плату, образ и профиль. Образ — из каталога (`get_boards`, `get_images_for_board`, `get_vendors`) или свой файл (`select_custom_image`, `classify_custom_image`).
2. `probe_custom_image` / `provision::probe_image` отвечает, можно ли применить настройки к образу (`ImageSupport`). Сжатый файл сначала распаковывается (`decompress_custom_image`): проба сжатого файла падает с тегом `[PROBE_COMPRESSED]`.
3. Перед записью форма «запечатывается»: `hash_password` даёт `$6$`-хеш, `derive_wifi_psk` — 64 hex. В профиль и на диск уходят только они (ADR 0004).
4. `flash_image` (`src-tauri/src/commands/operations.rs`) вызывает `autoconfig::effective_profile` (пустой профиль = без настроек) и `autoconfig::prepare_flash_copy`. Она копирует образ в `autoconfig_temp_dir()`, проверяет его `provision::probe_image` и вызывает `provision::provision_image` на копии. Кэш скачанного образа не меняется. Копия удаляется в `Drop`; при старте `sweep_stale_working_copies` чистит остатки.
5. Копия записывается на устройство платформенным кодом `src-tauri/src/flash/` (windows.rs, linux/, macos/). Прогресс — `get_download_progress`, `get_flash_progress`; отмена — `cancel_operation`.
6. На карте лежит drop-in к `armbian-firstrun.service`. При первом включении `firstboot.sh` (до сети) применяет настройки из `firstboot.env`, `post-network.sh` (после сети) доделывает Wi-Fi, запускает свой скрипт и стирает всё, что оставил имиджер.

Каталог: `catalog::load_active()` читает источник из настройки `catalog_source`, накладывает его на встроенный каталог и отдаёт типы upstream (`ApiVendor`, `ApiBoardSummary`, `ApiImage`). `images/mod.rs` ходит только в `catalog`, в API Armbian — никто, кроме QDL (`ARMBIAN_API_BASE`). Смена источника шлёт событие `wukong-catalog-changed`.

## Модули и границы

- `crates/classic-ext4` (MIT) — единственный, кто знает on-disk формат ext4. Снаружи нет блоков и inode: `open` / `open_read_only`, `exists`, `is_regular_file`, `mkdir`, `write_new_file`, `commit`. Всё меняется в памяти, на диск — в `commit`; ошибка до `commit` следов не оставляет. `open` отказывает (`Unsupported`) при блоке не 4096, `inode_size < 256`, `metadata_csum`, `gdt_csum`, `64bit`, `needs_recovery`, `bigalloc`, `inline_data` и при родителе с `EXT4_INDEX_FL`. Нет MBR или раздела 0x83 — `NotFound`; повреждение — `Corrupt`; симлинк в середине пути — `Unsupported` (`/lib` в образе симлинк: проверяй `/usr/lib/systemd/system/armbian-firstrun.service`). `commit` не атомарен при сбое записи — работать только с рабочей копией.
- `src-tauri/src/provision/` — превращает настройки в файлы на карте. Не зависит от остального приложения, поэтому `src-tauri/examples/provision.rs` подключает его через `#[path]`. API: `hash_password`, `derive_wifi_psk` (64-hex вход возвращается как есть), `probe_image`, `render_bundle` (чистая функция), `provision_image`, `is_empty`. Скрипты платы `src-tauri/device/firstboot.sh` и `post-network.sh` встроены `include_str!` в `bundle.rs`.
- `src-tauri/src/autoconfig.rs` — оркестратор: рабочая копия, отмена, очистка, теги ошибок `[AUTOCONFIG_NOT_ARMBIAN]`, `[AUTOCONFIG_ALREADY_BOOTED]`. `inject_into_bare_ext4_image` и `shell_quote` остались для ветки QDL и не затрагивают Wukong.
- `src-tauri/src/catalog/` — `mod.rs` (источник, встроенный каталог, слияние, `check`, кэш на 30 с) и `external.rs` (разбор и проверка `catalog.json` версии 1). Встроенный каталог всегда есть и содержит плату без образов; внешний дополняет его, не заменяя.
- `src-tauri/src/ssh_keys.rs` — разбор и загрузка `authorized_keys` (GitHub, GitLab, ссылка), команды `lookup_ssh_keys`, `fetch_ssh_keys`.
- Фронтенд: `src/App.tsx` → `src/components/{layout,settings,autoconfig,flash,modals}`; вызовы Tauri — `src/hooks/useTauri.ts` (плюс прямые `invoke` в `CatalogSection.tsx`); модель профиля — `src/config/profileEditorModel.ts`, `profileWizard.ts`, `autoconfig.ts`; общие типы — `src/types/index.ts`; плата по умолчанию — `src/config/wukong.ts` (`WUKONG_BOARD`, двойник встроенного каталога Rust).
- Не трогать без причины (минимальный дифф от upstream): `crates/armbian-write-conf`, `src-tauri/src/qdl`, `src-tauri/src/flash/macos`, код других плат. Новое — в новых файлах, правки upstream точечные.

## Профиль настроек

`AutoconfigConfig` (Rust: `provision/config.rs`; TS: `src/types/index.ts`; JSON — camelCase) хранит хеши и PSK, не открытые значения: `wifiPsk`, `userPasswordHash`, `rootPasswordHash`, `sshAuthorizedKeys` (текст), `overlays: string[]`, `userScript`, `hostname`, `keyboardLayout`, `wifiHidden`, `sshEnabled`, `sshPasswordAuth`, `sshRootLogin` и сетевые поля (`ethernetEnabled`, `wifi*`, `useStaticIp`, `static*`), `locale`, `timezone`, `user*`.

- Профили лежат в `settings.json` под ключом `autoconfig_profiles`. `EditorModel` держит хеш и PSK; открытые пароли — только в `EditorForm.secrets` (память), при сохранении и перед прошивкой проходят `sealEditorForm` → `hash_password` / `derive_wifi_psk`. Пустое поле пароля при существующем хеше значит «оставить».
- Незаполненное поле (например `sshEnabled`) шлётся как `null`, иначе профиль не считается пустым (`isFormEmpty`, `provision::is_empty`).
- Что карта получает, каталог `/etc/systemd/system/armbian-firstrun.service.d/`:

| Файл | Режим | Что |
|---|---|---|
| `10-wukong.conf` | 0644 | drop-in: `Before=NetworkManager.service ssh.service systemd-user-sessions.service`, `ExecStartPre=-/bin/bash …/firstboot.sh`, `TimeoutStartSec=15min` |
| `firstboot.sh` | 0755 | этап до сети |
| `post-network.sh` | 0755 | этап после сети |
| `firstboot.env` | 0600 | `KEY='value'`, `'` → `'\''` |
| `user-script.sh` | 0700 | свой скрипт, только если задан |

Ключи `firstboot.env`: `WK_HOSTNAME`, `WK_TIMEZONE`, `WK_LOCALE`, `WK_KEYBOARD`, `WK_WIFI_{ENABLED,SSID,PSK,COUNTRY,HIDDEN}`, `WK_STATIC_{ENABLED,IP,PREFIX,GATEWAY,DNS}`, `WK_USER_{NAME,REALNAME,HASH,SHELL}`, `WK_ROOT_HASH`, `WK_SSH_{ENABLED,PASSWORD_AUTH,ROOT_LOGIN,KEYS}`, `WK_OVERLAYS`. Незаданный ключ отсутствует, и скрипт пропускает шаг. Скрипты повторяемы: каждый шаг может выполниться дважды, сбой шага не останавливает следующие. Журнал платы — `/var/log/wukong-firstboot.log` (копия в `/var/log.hdd`).

## Каталог `catalog.json`, версия 1

`vendors[]` (`slug`, `name`), `boards[]` (`slug`, `name`, `vendor`, `soc`, `architecture`, `summary`, `image`), `images[]` (`board`, `name`, `release`, `kernel`, `size`, `sha256`, `supports_settings`, ровно одно из `url` и `file`). `file` — относительно папки каталога и не выходит за неё. В образах каталога `direct_url` — `http(s)` или `file://`, `sha_url` — `sha256:<hex>`. Пример и запуск — `docs/catalog/README.md`.

## Соглашения

- Секреты: пароль и ключ Wi-Fi в открытом виде живут только в состоянии формы до хеширования. Не на диске, не в профиле, не в логе, не в образе, не в аргументах команд на плате; `impl Debug` значений не показывает; сообщения `ProvisionError` не несут значений профиля.
- Паника в release = abort (`panic = "abort"`). В `classic-ext4` и `provision` нет `unwrap` / `expect` / `panic` на данных из образа — только `Result`.
- Пользовательские тексты — через i18n: ключи добавляются в `src/locales/en.json` и `ru.json`, остальные языки откатываются на en.
- Скрипты `*.sh` — только LF (`.gitattributes`), иначе bash на плате падает.
- Ошибки между Rust и фронтендом несут теги в тексте (`[AUTOCONFIG_ALREADY_BOOTED]`, `[PROBE_COMPRESSED]`, `[AUTOCONFIG_NOT_ARMBIAN]`); фронтенд разбирает их в `src/utils/errorUtils.ts`.
- `ImageSupport` в JSON: `{kind: "supported" | "notExt4" | "unsupportedFs" | "noFirstrunService" | "alreadyBooted", reason?}`.
- Debug-сборка включает `src-tauri/src/dev_scenarios` и `commands/dev_scenarios.rs` (имитация дисков и прошивки) и мост `tauri-plugin-mcp-bridge`; в release их нет. Панель сценариев в Vite — флаг `__DEV_SCENARIOS__` (`npm run dev`, `build:dev`).

## Тестовые швы

- `classic-ext4`: файл-образ на входе; фикстура `crates/classic-ext4/tests/fixtures/classic.img.gz` (пересборка — `crates/classic-ext4/scripts/build-fixture.sh`, нужен Docker); `crates/classic-ext4/scripts/fsck-check.sh` — `e2fsck` на выходе; `tests/real_image.rs` работает на настоящем образе из `WUKONG_TEST_IMAGE` (без него пропускается).
- `provision`: `render_bundle` — чистая функция; скрипты платы — `bash -n` и `scripts/e2e/device-test.sh` на заглушках.
- Сквозная проверка: `scripts/e2e/run.sh` — `examples/provision.rs` применяет настройки к копии образа, QEMU `-M orangepi-pc` в Docker загружает её, `guest-checks.sh` сверяет итог. Коды выхода: 0 всё прошло, 1 сбой проверки, 2 не стартовало, 3 загрузка не подтверждена. Проверка на настоящей плате — `docs/hardware-checklist.md`.
