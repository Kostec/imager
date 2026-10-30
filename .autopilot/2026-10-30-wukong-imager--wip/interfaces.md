# Интерфейсы и правила проекта

Этот файл каждый исполнитель читает первым. Факты о коде и устройстве — в `notes.md`.

## Правила проекта

- Это форк `armbian/imager` (GPL-2.0-or-later). Лицензия, копирайты и упоминание оригинала сохраняются. Новый crate `crates/classic-ext4` — MIT, как `armbian-write-conf`.
- Минимальный дифф от upstream: код других плат, QDL и macOS не удаляем и не правим сверх нужного для сборки. Новое — в новых файлах, правки upstream — точечные.
- Работа в ветке `wukong`. Push только по команде пользователя.
- Секреты: пароль и ключ Wi-Fi в открытом виде живут только в состоянии формы, до хеширования. Никогда — на диске, в профиле, в логе, в образе, в аргументах команд на устройстве. `impl Debug` не показывает значения.
- Пользовательские тексты — через i18n: ключи добавляются в `en.json` и `ru.json`, остальные языки падают на en.
- Паника в release = abort (`panic=abort`). В `classic-ext4` и `provision` нет `unwrap`/`expect`/`panic` на данных из образа; всё — `Result`.
- Проверка перед сдачей таска: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` в затронутом crate; для фронтенда `npm run lint`, `npm run build`. В `src-tauri` для cargo нужен каталог `dist/`.

## Стек и команды

- Tauri 2, Rust 1.99 (в Git Bash: `export PATH="$PATH:/c/Users/work/.cargo/bin"`), Node 24, React 19, TypeScript, Vite. MSVC 14.44 установлен. Docker доступен, WSL Ubuntu-22.04 есть.
- Установка: `npm ci`. Запуск: `npm run tauri:dev`, требует повышенного терминала (`requireAdministrator`).
- Проверка фронтенда: `npm run lint && npm run build`.
- Проверка backend: `mkdir -p dist && cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. Если тестовый бинарник просит повышения прав из-за манифеста, сообщи, а не обходи.
- Один файл тестов: `cargo test --test <имя>`.
- Новый crate: `cargo test --manifest-path crates/classic-ext4/Cargo.toml`.
- Не трогать: `D:\Projects\Wukong\wukongpi_build`, исходный образ (только чтение), `crates/armbian-write-conf`, `src-tauri/src/qdl`, `src-tauri/src/flash/macos`.

## Границы, решённые в спецификации

### 1. `crates/classic-ext4` (новый)

Владеет: разбором MBR, суперблока и дескрипторов классического ext4, аллокацией, вставкой записей каталога.
Прячет: всё on-disk. Снаружи нет блоков и inode.

```rust
pub struct ClassicExt4 { /* ... */ }
impl ClassicExt4 {
    pub fn open(image: &Path) -> Result<Self, Ext4Error>;      // rootfs = первый раздел 0x83 в MBR, RW
    pub fn open_read_only(image: &Path) -> Result<Self, Ext4Error>;
    pub fn is_regular_file(&self, path: &str) -> Result<bool, Ext4Error>;
    pub fn exists(&self, path: &str) -> Result<bool, Ext4Error>; // следует по симлинкам внутри rootfs не нужно: lstat
    pub fn mkdir(&mut self, path: &str, mode: u16) -> Result<(), Ext4Error>;   // root:root, родитель существует
    pub fn write_new_file(&mut self, path: &str, data: &[u8], mode: u16) -> Result<(), Ext4Error>; // только новый файл, root:root
    pub fn commit(self) -> Result<(), Ext4Error>;              // сброс счётчиков и flush
}
pub enum Ext4Error { Io(io::Error), Unsupported(String), NotFound(String), AlreadyExists(String), NoSpace, Corrupt(String) }
```

Откат: `open` отказывает (`Unsupported`), если блок не 4096, inode_size < 256, есть `metadata_csum`, `gdt_csum`, `64bit`, `needs_recovery`, `bigalloc`, `inline_data`. Родитель с `EXT4_INDEX_FL` — `Unsupported`. Всё меняется в памяти, на диск только в `commit`: ошибка до `commit` не оставляет следов.

### 2. `src-tauri/src/provision/` (новый модуль)

Владеет: превращением настроек в файлы на карте и скриптами устройства (`src-tauri/device/*.sh`, `include_str!`).

```rust
pub fn hash_password(plain: &str) -> Result<String, ProvisionError>;              // $6$ SHA-512 crypt
pub fn derive_wifi_psk(ssid: &str, passphrase: &str) -> Result<String, ProvisionError>; // 64 hex; 64-hex вход возвращается как есть
pub fn probe_image(image: &Path) -> Result<ImageSupport, ProvisionError>;
pub enum ImageSupport { Supported, NotExt4, UnsupportedFs(String), NoFirstrunService, AlreadyBooted }
pub fn render_bundle(cfg: &AutoconfigConfig) -> Result<Vec<BundleFile>, ProvisionError>;
pub struct BundleFile { pub path: String, pub mode: u16, pub data: Vec<u8> }
pub fn provision_image(image: &Path, cfg: &AutoconfigConfig) -> Result<(), ProvisionError>;
pub fn is_empty(cfg: &AutoconfigConfig) -> bool;
```

Каталог на карте: `/etc/systemd/system/armbian-firstrun.service.d/`:

| Файл | Режим | Что |
|---|---|---|
| `10-wukong.conf` | 0644 | drop-in: `[Unit] Before=NetworkManager.service ssh.service`, `[Service] ExecStartPre=-/bin/bash <dir>/firstboot.sh`, `TimeoutStartSec=15min` |
| `firstboot.sh` | 0755 | этап до сети |
| `post-network.sh` | 0755 | этап после сети |
| `firstboot.conf` | 0600 | `KEY='value'`, значения в одинарных кавычках, `'` → `'\''` |
| `user-script.sh` | 0700 | свой скрипт, только если задан |

Ключи `firstboot.conf`: `WK_HOSTNAME`, `WK_TIMEZONE`, `WK_LOCALE`, `WK_KEYBOARD`, `WK_WIFI_ENABLED`, `WK_WIFI_SSID`, `WK_WIFI_PSK`, `WK_WIFI_COUNTRY`, `WK_WIFI_HIDDEN`, `WK_STATIC_ENABLED`, `WK_STATIC_IP`, `WK_STATIC_PREFIX`, `WK_STATIC_GATEWAY`, `WK_STATIC_DNS`, `WK_USER_NAME`, `WK_USER_REALNAME`, `WK_USER_HASH`, `WK_USER_SHELL`, `WK_ROOT_HASH`, `WK_SSH_ENABLED`, `WK_SSH_PASSWORD_AUTH`, `WK_SSH_ROOT_LOGIN`, `WK_SSH_KEYS` (многострочная), `WK_OVERLAYS`. Незаданный ключ отсутствует; скрипт этот шаг пропускает.

### 3. `src-tauri/src/autoconfig.rs` (upstream, правится)

Остаётся оркестратором: `prepare_flash_copy`, `prepare_tracked`, `WorkingCopy`, отмена, очистка. `inject_into_image` заменяется вызовом `provision::provision_image`; маркер `armbian-firstlogin` заменяется `provision::probe_image`. `render_preset` удаляется вместе с записью в `/root/.not_logged_in_yet`; ветка QDL (`inject_into_bare_ext4_image`) остаётся нетронутой.

`AutoconfigConfig` (Rust и TS, camelCase) — поля:
оставить: `ethernetEnabled, wifiEnabled, wifiSsid, wifiCountryCode, useStaticIp, staticIp, staticMask, staticGateway, staticDns, locale, timezone, userName, userShell, userRealName`;
заменить: `wifiKey`→`wifiPsk`, `userPassword`→`userPasswordHash`, `rootPassword`→`rootPasswordHash`, `userKeyUrl`+`rootKeyUrl`→`sshAuthorizedKeys: string`;
удалить: `applyNetwork, langBasedOnLocation, remoteConfigUrl`;
новые: `hostname, keyboardLayout, wifiHidden, sshEnabled, sshPasswordAuth, sshRootLogin, overlays: string[], userScript: string`.

### 4. `src-tauri/src/catalog/` (новый модуль)

```rust
pub enum CatalogSource { BuiltIn, Url(String), Folder(PathBuf) }
pub fn current_source() -> CatalogSource;              // настройка `catalog_source`
pub async fn load(source: &CatalogSource) -> Result<Catalog, CatalogError>;
pub struct Catalog { vendors: Vec<ApiVendor>, boards: Vec<ApiBoardSummary>, images: HashMap<String, Vec<ApiImage>> } // уже типы upstream
```

`fetch_boards`, `fetch_images_for_board`, `fetch_vendors` из `images/mod.rs` ходят только сюда. В Armbian API никто не ходит. Встроенный каталог всегда присутствует; внешний дополняет его.

`catalog.json`, версия 1:

```json
{ "version": 1,
  "vendors": [{ "slug": "wukong", "name": "Wukong" }],
  "boards":  [{ "slug": "wukongpi", "name": "Wukong Pi Zero", "vendor": "wukong", "soc": "H2+", "image": "wukongpi.png" }],
  "images":  [{ "board": "wukongpi", "name": "Bullseye 5.15", "release": "bullseye", "kernel": "5.15.93",
                "url": "https://…/x.img.xz", "file": "x.img.xz", "sha256": "…", "size": 123, "supports_settings": true }] }
```

`url` и `file` взаимоисключающи; `file` относительно папки каталога и не выходит за неё.

### 5. Команды Tauri, новые

`hash_password(plain) -> String`, `derive_wifi_psk(ssid, passphrase) -> String`, `probe_custom_image(path) -> ImageSupport`, `get_catalog_source() / set_catalog_source(value)`, `fetch_ssh_keys(source, value) -> String` (текст строк `authorized_keys`).

### 6. Фронтенд

`src/config/autoconfig.ts`, `profileEditorModel.ts`, `types/index.ts` следуют полям §3. Хранилище профилей прежнее (`autoconfig_profiles` в `settings.json`), но `EditorModel` держит открытый пароль только в памяти; при сохранении и перед прошивкой вызывает `hash_password`/`derive_wifi_psk`. Пустое поле пароля при существующем хеше = «оставить как есть».

## Тестовые швы

- `classic-ext4`: файл-образ на входе, `e2fsck -fn` на выходе. Фикстура `crates/classic-ext4/tests/fixtures/classic.img.gz` (собрана теми же `mkfs.ext4 -O ^64bit,^metadata_csum`).
- `provision`: `render_bundle` — чистая функция; скрипты устройства проверяются `bash -n`, `shellcheck` и прогоном в Docker на распакованном rootfs.
- e2e: `scripts/e2e/` — dev-утилита `examples/provision.rs` + QEMU `-M orangepi-pc` в Docker, образ из `WUKONG_TEST_IMAGE`.

## Что построили таски

<!-- дополняется по мере сборки -->
