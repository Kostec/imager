# 01 — Программа становится Wukong Imager и собирается под Windows и Linux

**Требования:** R01, R02, R04, R10, R14, R15, R16, R18, R19, R21, R22i, R23i, G03, G09, G10
**Зависит от:** —
**Зона:** `src/` · `src-tauri/` · `scripts/build/` · `.github/` · `public/` · `index.html` · `package.json` · `README.md`
**Волна:** 1
**Ревью:** да — фундамент
**Модель:** сильная

## Что должно заработать

Запускается приложение Wukong Imager: сразу видна плата Wukong Pi Zero, можно выбрать `.img`, `.iso` или сжатый образ с диска, выбрать карту, подтвердить и прошить с проверкой. Настройки на этом этапе скрыты. Программа не обращается к серверам Armbian, работает без интернета, собирается без ключей подписи на Windows и в Ubuntu 22.04.

## Из брифа, дословно

> «аналог Raspberry Pi Imager, но для Wukong Pi»
> «Программа должна быть кроссплатформенной и поддерживать запуск как из под Windows, так и из под Linux»
> «каталог… пока что его можно отключить и работать только с Wukong»
> «можно сделать его форк и добавить поддержку нужной платы»

## Разделы спецификации

Истории 1–6, 8, 38, 39, 42, 43, 48–50. Решения: «Встроенный каталог», «Проверка соединения», «Внешние запросы убираем», «Идентификатор», «QDL и macOS». `notes.md` §2, 4, 5, 6.

## Критерии приёмки

- [ ] `productName`, окно, бинарник, пакет и `identifier` — по спецификации; `config::app::NAME` = `wukong-imager`
- [ ] Значок и логотип заменены нейтральной заглушкой, логотипы Armbian не показываются; оригинал и лицензия указаны в «О программе» и README
- [ ] Модуль `catalog` со встроенным каталогом (`CatalogSource::BuiltIn`); `fetch_boards/fetch_images_for_board/fetch_vendors` не ходят в Armbian API
- [ ] При старте шаги производителя и платы не показываются, активен выбор образа; `.iso` записывается побайтово; после сброса возвращается к Wukong
- [ ] Офлайн нет экрана «нет сети»; `check_connectivity` не ходит на `api.armbian.com`
- [ ] Updater, `paste`, MOTD, Google Fonts и ссылки Armbian не используются; `createUpdaterArtifacts` выключен
- [ ] `grep -r "armbian.com" src src-tauri/src src-tauri/*.json` даёт только комментарии лицензии и инертный код QDL
- [ ] `schedule` снят с `data-sync-locales`, `maintenance-delete-releases`, `maintenance-pr-check-cleanup`
- [ ] На Windows `npm run tauri:build` даёт установщик без ключей подписи
- [ ] Скрипт `scripts/build/linux-docker.sh` собирает `.deb` и `.AppImage` в `ubuntu:22.04`
- [ ] В `en.json` и `ru.json` переписаны тексты с «Armbian» про поведение программы
