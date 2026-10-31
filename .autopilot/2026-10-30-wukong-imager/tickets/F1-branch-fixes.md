# F1 — Правки по ревью ветки: имя.local, причина сбоя Wi-Fi, лог, затирание, форма, переводы

**Требования:** G02, G07, R26i, R06, G09
**Зависит от:** —
**Зона:** `src-tauri/device/` · `scripts/e2e/` · `docs/hardware-checklist.md` · `docs/adr/0003-firstrun-dropin.md` · `src/config/profileEditorModel.ts` · `src/config/profileEditorModel.devtest.ts` · `src/config/profileWizard.ts` · `src/components/settings/editor/` · `src/locales/`
**Волна:** 5
**Ревью:** да — ремонт ветки
**Модель:** сильная

## Что должно заработать

Плата с заданным именем отвечает на `имя.local`, причина сбоя Wi-Fi видна в логе, лог переживает выдёргивание питания, настройки не остаются на карте молча, ошибка фиксированного IP видна в форме, в других языках нет ложных текстов про Armbian.

## Из брифа, дословно

> «Твой набор устраивает»
> «[при сбое Wi-Fi] 2. ОК»
> «[название] 4. Ок»

## Разделы спецификации

Истории 15, 16, 37. `interfaces.md` §2, 3, T03, T06. `docs/hardware-checklist.md` §8, §11.

## Критерии приёмки

- [ ] **Avahi.** `src-tauri/device/post-network.sh:60-66`, `scripts/e2e/guest-checks.sh:110-114`. На образе 23.02.2 с интернетом после первой загрузки в логе `OK avahi-daemon installed` и `systemctl is-active avahi-daemon` = active; apt работает со списком без bullseye-backports и security (обход из `docs/hardware-checklist.md` §8), таймаут не обрывает dpkg; в онлайн-прогоне e2e отсутствие avahi — FAIL; известная проблема убрана из §8.
- [ ] **Причина сбоя Wi-Fi.** `post-network.sh:125-128`. При `WK_WIFI_ENABLED=1` post-network ограниченное время ждёт активации `wukong-wifi` и пишет `OK wifi` или `FAIL wifi: <состояние и причина NetworkManager>`, без значений из `firstboot.env`, и не блокирует остальное; e2e с несуществующей сетью проверяет строку FAIL.
- [ ] **Лог при обрыве питания.** `firstboot.sh:15`, `post-network.sh:13`. После `kill -9` QEMU и холодной загрузки в `/var/log/wukong-firstboot.log` есть `firstboot: finished` и `post-network: finished` (копия в `/var/log.hdd` и `sync` в конце каждого этапа, syncFromDisk вернёт её); абзац §11 чек-листа обновлён.
- [ ] **Затирание.** `post-network.sh:91,109-111,72`. `disable` и удаление unit только после проверки, что каталога нет; иначе лог `FAIL settings not wiped` и повтор на следующей загрузке. Сбой `install` в `stage_user_script` пишется в лог.
- [ ] **Маска фиксированного IP.** `src/config/profileEditorModel.ts:291-293`, `src/config/profileWizard.ts:74` против `src-tauri/src/provision/bundle.rs:258`. В редакторе и мастере пустая маска при fixed — ошибка у поля, Next и сохранение заблокированы; devtest на это есть.
- [ ] **Переводы.** `src/locales/*.json` кроме en и ru. Ключи, переписанные в en, удалены из этих 18 файлов, работает откат на en; значения со словом Armbian остались только в `about.upstream` и `about.credits`.
- [ ] `scripts/e2e/run.sh` на настоящем образе проходит онлайн, включая новые проверки; `scripts/build/linux-docker.sh --test`, `npm run lint`, `npm run build` чисты; `bash -n` и `shellcheck` чисты.
