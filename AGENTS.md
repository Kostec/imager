<!-- autopilot:start -->
# Wukong Imager

Tauri-приложение (Windows, Linux) для разработчиков проекта: записывает образ на SD-карту Wukong Pi Zero и кладёт на неё настройки первого включения; форк `armbian/imager`. Только GUI; CLI нет, есть dev-утилита `src-tauri/examples/provision.rs`.

Архитектура и соглашения: `docs/architecture.md`. Почему так: `docs/adr/`. Словарь: `CONTEXT.md`.

## Команды

Из корня репозитория, в Git Bash; cargo в PATH: `export PATH="$PATH:$HOME/.cargo/bin"`. Нужны Node >=20.19 (`package.json`), Rust >=1.85 (`rust-version` в `src-tauri/Cargo.toml`; `DEVELOPMENT.md` с Rust 1.77.2 и клоном `armbian/imager` устарел), Docker для Linux и e2e.

- Установка: `npm ci`
- Проверка фронтенда: `npm run lint` и `npm run build` (кладёт `dist/`; cargo в `src-tauri` без `dist/` не соберётся)
- Модель профиля (TS, без фреймворка, 26 проверок): `T=$(mktemp -d) && npx rolldown src/config/profileEditorModel.devtest.ts --format esm --platform node --file $T/model.mjs && node $T/model.mjs`
- Запуск с горячей перезагрузкой: `npm run tauri:dev` только из терминала администратора; без прав — `npm run dev` (Vite, :5173) и параллельно запустить debug-exe
- Debug-exe без установщика (~3 мин): `npx tauri build --debug --no-bundle --config src-tauri/tauri.debug.conf.json` → `src-tauri\target\debug\wukong-imager.exe`
- Релиз Windows: `npm run tauri:build` → `.msi` и `-setup.exe` в `src-tauri\target\release\bundle\`
- Релиз Linux (Docker): `bash scripts/build/linux-docker.sh` → `.AppImage` и `.deb` в `releases/linux/`
- Тесты `src-tauri` (cargo test, fmt, clippy в ubuntu:22.04): `bash scripts/build/linux-docker.sh --test`; один тест: `bash scripts/build/linux-docker.sh --test -- <часть имени>`
- Backend на Windows (тесты здесь не идут, см. камни): `cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --no-run`
- Crate `classic-ext4` (~4 мин): `cargo test --manifest-path crates/classic-ext4/Cargo.toml`; один файл тестов: `... --test refuse` (есть `create`, `refuse`, `real_image`)
- Скрипты платы: `bash -n src-tauri/device/firstboot.sh && bash -n src-tauri/device/post-network.sh`
- Утилита без GUI: `cd src-tauri && cargo run --example provision -- probe <образ>` (также `apply <образ> <settings.json>`, `render <settings.json> <каталог>`); на Windows манифест администратора может помешать запуску, e2e собирает её в Docker
- Сквозная проверка в QEMU (десятки минут, первое приглашение входа ~437 с): `E2E_BOOT_TIMEOUT=1800 E2E_SETTLE_TIMEOUT=900 WUKONG_TEST_IMAGE=<путь к .img> bash scripts/e2e/run.sh`; `--offline` — только проверки образа, без QEMU. Коды выхода: 0, 1 сбой, 2 не стартовало, 3 загрузка не подтверждена

## Где что

- `src-tauri/src/main.rs` — плагины и список команд (`generate_handler!`); новая команда регистрируется здесь, реализация в `src-tauri/src/commands/`
- `src-tauri/src/provision/` и `src-tauri/device/*.sh` — настройки → файлы на карте и скрипты платы (чаще всего правят `bundle.rs` и скрипты)
- `src-tauri/src/autoconfig.rs` + `commands/operations.rs` (`flash_image`) — рабочая копия образа с настройками перед записью
- `crates/classic-ext4/` — запись в ext4 образа без монтирования
- `src-tauri/src/catalog/` и `docs/catalog/` — источники каталога образов (встроенный, URL, папка)
- `src/config/profileEditorModel.ts`, `profileWizard.ts`, `autoconfig.ts` и `src/hooks/useTauri.ts` — форма профиля и вызовы Tauri; тексты — `src/locales/en.json`, `ru.json`
- `docs/hardware-checklist.md` — проверка на настоящей плате; `scripts/e2e/` — сквозная проверка

## Подводные камни

- Windows-exe требует администратора (`src-tauri/app.manifest`). Из обычного терминала `npm run tauri:dev` и `cargo test` в `src-tauri` падают с os error 740. Запущенный exe не остановить и не управлять окном из не повышенного процесса (Access is denied); при запущенном exe `tauri:dev` падает с os error 5.
- Мост `tauri-plugin-mcp-bridge` (только debug, `127.0.0.1:9224`): в сборке с `tauri.debug.conf.json` нет `withGlobalTauri`, и его `execute_js` висит до таймаута; `tauri.dev.conf.json` (для `tauri:dev`) его включает.
- Linux не собрать ни на Windows, ни в WSL (нет webkit2gtk и Rust) — только `linux-docker.sh`; он сам берёт контекст `desktop-linux` и тома `wukong-imager-cargo`, `wukong-imager-target` (`--clean` сбрасывает).
- Скрипты `*.sh` — только LF (`.gitattributes`): с CRLF bash на плате падает.
- После сборки git может показать `src-tauri/Cargo.toml` изменённым только из-за окончаний строк (diff пустой).
- `node scripts/locales/check-parity.mjs` падает (exit 1, 1962 ошибки): языки кроме en и ru откатываются на en, а проверка ждёт полных переводов. Не чинить правкой переводов.
- Настройки приложения, профили, источник каталога и кэш общие между запусками: `%APPDATA%\io.github.kostec.wukongimager` (store `settings.json`: `autoconfig_profiles`, `catalog_source`).
- Открытые пароли и ключ Wi-Fi не должны попадать на диск, в профиль и лог (`docs/adr/0004-secrets-as-hashes.md`); незаполненные поля профиля шлют как `null`.
- Не править без причины (минимальный дифф от upstream): `crates/armbian-write-conf`, `src-tauri/src/qdl`, `src-tauri/src/flash/macos`. Лицензия GPL-2.0-or-later и упоминание upstream остаются.
- `.env.example` и его `VITE_*` — остаток upstream, код их не читает.

## Окружение

- `WUKONG_TEST_IMAGE` — образ для e2e и для `crates/classic-ext4/tests/real_image.rs` (без него тест пропускается); образ только читается
- `E2E_WORK_DIR`, `E2E_DTB`, `E2E_BOOT_TIMEOUT`, `E2E_SETTLE_TIMEOUT`, `E2E_REUSE_BUILD` — настройка `scripts/e2e/run.sh` (каталог отчёта, дерево устройств, таймауты, не пересобирать утилиту)
- `CLASSIC_EXT4_KEEP_DIR` — куда `real_image.rs` оставляет копию образа
- `ARMBIAN_API_BASE` — база API Armbian, нужна только коду QDL
- `ARMBIAN_TEST_IMAGE` — образ для тестов `crates/armbian-write-conf`
- `PKEXEC_UID`, `SUDO_UID`, `SUDO_USER` — Linux: код читает их, чтобы найти домашний каталог пользователя при запуске под root
- `OPENAI_API_KEY`, `OPENAI_MODEL`, `OPENAI_API` — только `scripts/locales/sync-locales.js` (автоперевод)

## Как здесь работает Autopilot

Сборка ведётся навыком `/autopilot`. Требования, спецификация и таски — в `.autopilot/`.
Прогресс — `.autopilot/dashboard.html`. Требование из `manifest.md` может снять
только пользователь. Прерванную сборку продолжает «продолжи автопилот».

### Правка без автопилота

Мелкий баг или доводка после сборки — обычная сессия, без прогона.

Сначала прочитай:
1. `.autopilot/PROGRESS.md` — что не доделано, известные баги, о чём договорились с пользователем.
2. Что правили после последней сборки — первая строка таблицы в «История git».
3. Только если правка их касается: почему сделано так — `docs/adr/`; что значат слова — `CONTEXT.md`;
   что именно просили — `manifest.md` в папке сборки (указана в начале `PROGRESS.md`).

После правки:
- коммит — по правилам из «История git»;
- строка в «Правки без автопилота» в `PROGRESS.md`: дата — что — почему;
- починенный пункт из «Не доделано» или «Известные баги» — удали;
- договорились с пользователем о том, что следующая сборка не должна пересматривать, —
  строка в «Договорились», его словами.

### История git

Целиком не читай — бери срез под вопрос:

| Вопрос | Команда |
|---|---|
| что правили после последней сборки | `git log --oneline "$(git log -1 --format=%h --grep='^Autopilot: сборка сдана')"..HEAD -- . ':(exclude).autopilot'` |
| всё, что делали ради требования | `git log --oneline --grep="R02"` |
| история файла | `git log --oneline -10 -- <файл>` |
| когда появился или исчез код | `git log --oneline -S "<фрагмент>"` |
| почему строка такая | `git blame -L <с>,<по> <файл>`, затем `git show <sha>` |
| что внутри коммита | `git show --stat <sha>` |

Срез длиннее ~30 коммитов — отдай его субагенту и возьми выжимку в десять строк.

Коммит: заголовок — что (`fix: даты в одном формате`), тело — почему, трейлер
`--trailer "Requirements: R02"`, если правка касается требования из `manifest.md`.
Заголовок `Autopilot: сборка сдана` пишет только автопилот — по нему находят последнюю сборку.
<!-- autopilot:end -->
