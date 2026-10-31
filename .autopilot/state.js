window.STATE =
{
  "version": 2,
  "slug": "wukong-imager",
  "dir": "2026-10-30-wukong-imager",
  "title": "Wukong Imager — прошивальщик SD с первичной настройкой",
  "mode": "interview",
  "depth": "deep",
  "tier": "T2",
  "briefFile": "2026-10-30-brief.md",
  "memoryFile": "AGENTS.md",
  "memoryOwner": "autopilot",
  "skillDir": "C:/Users/work/.agents/skills/autopilot",
  "baseCommit": "9a1fa2e",
  "startedAt": "2026-10-30T09:49:18+03:00",
  "updatedAt": "2026-10-31T10:02:43+03:00",
  "finishedAt": "2026-10-31T10:02:43+03:00",
  "stages": [
    {
      "id": "preflight",
      "status": "done",
      "startedAt": "2026-10-30T09:49:18+03:00",
      "finishedAt": "2026-10-30T09:49:38+03:00"
    },
    {
      "id": "manifest",
      "status": "done",
      "startedAt": "2026-10-30T09:49:38+03:00",
      "finishedAt": "2026-10-30T09:50:05+03:00"
    },
    {
      "id": "briefing",
      "status": "done",
      "startedAt": "2026-10-30T09:50:05+03:00",
      "finishedAt": "2026-10-30T10:32:05+03:00"
    },
    {
      "id": "spec",
      "status": "done",
      "startedAt": "2026-10-30T10:32:05+03:00",
      "finishedAt": "2026-10-30T10:52:31+03:00"
    },
    {
      "id": "plan",
      "status": "done",
      "startedAt": "2026-10-30T10:52:31+03:00",
      "finishedAt": "2026-10-30T10:55:30+03:00"
    },
    {
      "id": "build",
      "status": "done",
      "startedAt": "2026-10-30T10:55:30+03:00",
      "finishedAt": "2026-10-30T17:20:15+03:00"
    },
    {
      "id": "review",
      "status": "done",
      "startedAt": "2026-10-30T17:20:15+03:00",
      "finishedAt": "2026-10-30T22:13:14+03:00"
    },
    {
      "id": "final",
      "status": "done",
      "startedAt": "2026-10-30T22:13:14+03:00",
      "finishedAt": "2026-10-31T10:02:43+03:00"
    }
  ],
  "requirements": {
    "total": 36,
    "done": 33,
    "inTicket": 0,
    "inSpec": 0,
    "placeholder": 0,
    "deferred": 0,
    "dropped": 3
  },
  "tickets": [
    {
      "id": "01",
      "title": "Программа становится Wukong Imager и собирается под Windows и Linux",
      "requirements": [
        "R01",
        "R02",
        "R04",
        "R10",
        "R14",
        "R15",
        "R16",
        "R18",
        "R19",
        "R21",
        "R22i",
        "R23i",
        "G03",
        "G09",
        "G10"
      ],
      "blockedBy": [],
      "wave": 1,
      "zone": [
        "src/",
        "src-tauri/",
        "scripts/build/",
        ".github/",
        "public/",
        "index.html",
        "package.json",
        "README.md"
      ],
      "review": "да — фундамент",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-30T10:55:31+03:00",
      "finishedAt": "2026-10-30T14:03:09+03:00",
      "retries": 0,
      "repairs": 1,
      "repairFindings": [
        "AppImage из linux-docker.sh проходит appimage-unbundle-wayland.sh; тест отказа Url/Folder убирает T05"
      ],
      "tests": {
        "passed": 229,
        "failed": 0
      },
      "commit": "bb7cc83"
    },
    {
      "id": "02",
      "title": "Запись файлов в классический ext4 образа Wukong",
      "requirements": [
        "R14",
        "R25i"
      ],
      "blockedBy": [],
      "wave": 1,
      "zone": [
        "crates/classic-ext4/"
      ],
      "review": "нет",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-30T10:55:31+03:00",
      "finishedAt": "2026-10-30T11:44:23+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 27,
        "failed": 0
      },
      "commit": "b75707a"
    },
    {
      "id": "03",
      "title": "Настройки ложатся на карту и применяются при первой загрузке",
      "requirements": [
        "R06",
        "R07",
        "R08",
        "R11",
        "R12",
        "R13",
        "R24i",
        "R25i",
        "R26i",
        "G01",
        "G02",
        "G07",
        "G08"
      ],
      "blockedBy": [
        "01",
        "02"
      ],
      "wave": 2,
      "zone": [
        "src-tauri/src/provision/",
        "src-tauri/device/",
        "src-tauri/src/autoconfig.rs",
        "src-tauri/src/commands/operations.rs",
        "src-tauri/src/commands/custom_image.rs",
        "src-tauri/src/ssh_keys.rs",
        ".gitattributes"
      ],
      "review": "да — пароли и секреты",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-30T12:53:50+03:00",
      "finishedAt": "2026-10-30T15:51:32+03:00",
      "retries": 0,
      "repairs": 1,
      "repairFindings": [
        "firstboot.conf переименовать в firstboot.env; .iso через probe_image"
      ],
      "tests": {
        "passed": 301,
        "failed": 0
      },
      "commit": "8e644f8"
    },
    {
      "id": "04",
      "title": "Экран настроек, профили и подтверждение",
      "requirements": [
        "R06",
        "R07",
        "R08",
        "R09",
        "R10",
        "R26i",
        "G02",
        "G08"
      ],
      "blockedBy": [
        "03"
      ],
      "wave": 3,
      "zone": [
        "src/components/autoconfig/",
        "src/components/settings/editor/",
        "src/components/layout/DevicePanel.tsx",
        "src/config/autoconfig.ts",
        "src/config/profileEditorModel.ts",
        "src/config/profileWizard.ts",
        "src/hooks/useSettings.ts",
        "src/types/index.ts",
        "src/config/constants.ts",
        "src/components/settings/SettingsNav.tsx",
        "src/components/layout/HomePage.tsx",
        "src/locales/en.json",
        "src/locales/ru.json",
        "src/components/settings/ProfileEditor.tsx",
        "src/hooks/useProfileWizard.ts",
        "src/components/flash/FlashProgress.tsx",
        "src/styles/profile.css",
        "src/hooks/useTauri.ts",
        "src/utils/errorUtils.ts",
        "src/App.tsx",
        "src/hooks/useFlashOperation.ts"
      ],
      "review": "да — пароли и секреты",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-30T14:38:56+03:00",
      "finishedAt": "2026-10-30T15:11:58+03:00",
      "retries": 0,
      "repairs": 1,
      "repairFindings": [
        "SSH без пользователя и с закрытым root не сохраняется; ключом считается только строка с типом, PEM отказывается"
      ],
      "tests": {
        "passed": 25,
        "failed": 0
      },
      "commit": "bca7d74"
    },
    {
      "id": "05",
      "title": "Внешний каталог: URL или папка на диске",
      "requirements": [
        "G04",
        "G05"
      ],
      "blockedBy": [
        "01"
      ],
      "wave": 2,
      "zone": [
        "src-tauri/src/catalog/",
        "src-tauri/src/images/",
        "src-tauri/src/download.rs",
        "src-tauri/src/commands/board_queries.rs",
        "src/components/settings/CatalogSection.tsx",
        "src/components/layout/ImagePanel.tsx",
        "src/components/layout/HomePage.tsx",
        "src/components/layout/Header.tsx",
        "src/App.tsx",
        "src/locales/en.json",
        "src/locales/ru.json",
        "docs/catalog/"
      ],
      "review": "нет",
      "model": "обычная",
      "status": "done",
      "startedAt": "2026-10-30T12:53:50+03:00",
      "finishedAt": "2026-10-30T14:38:49+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 294,
        "failed": 0
      },
      "commit": "45e310f"
    },
    {
      "id": "06",
      "title": "Сквозная проверка и чек-лист для платы",
      "requirements": [
        "G06",
        "R11",
        "R20",
        "G01"
      ],
      "blockedBy": [
        "03",
        "04"
      ],
      "wave": 4,
      "zone": [
        "scripts/e2e/",
        "docs/hardware-checklist.md",
        "docs/adr/"
      ],
      "review": "нет",
      "model": "сильная",
      "status": "done",
      "startedAt": "2026-10-30T15:46:45+03:00",
      "finishedAt": "2026-10-30T17:20:11+03:00",
      "retries": 0,
      "repairs": 0,
      "tests": {
        "passed": 46,
        "failed": 0
      },
      "commit": "0907d3b"
    },
    {
      "id": "F1",
      "title": "Правки по ревью ветки: имя.local, причина сбоя Wi-Fi, лог, затирание, форма, переводы",
      "requirements": [
        "G02",
        "G07",
        "R26i",
        "R06",
        "G09"
      ],
      "blockedBy": [],
      "wave": 5,
      "zone": [
        "src-tauri/device/",
        "scripts/e2e/",
        "docs/hardware-checklist.md",
        "docs/adr/0003-firstrun-dropin.md",
        "src/config/profileEditorModel.ts",
        "src/config/profileEditorModel.devtest.ts",
        "src/config/profileWizard.ts",
        "src/components/settings/editor/",
        "src/locales/"
      ],
      "review": "да — ремонт ветки",
      "model": "сильная",
      "status": "done",
      "retries": 0,
      "repairs": 1,
      "startedAt": "2026-10-30T17:28:54+03:00",
      "repairFindings": [
        "e2e: после kill -9 и холодной загрузки нет /var/log/wukong-firstboot.log, own script count пуст"
      ],
      "finishedAt": "2026-10-30T22:13:12+03:00",
      "tests": {
        "passed": 301,
        "failed": 0
      },
      "commit": "4e7e7b1"
    }
  ],
  "tests": {
    "passed": 301,
    "failed": 0
  },
  "debt": {
    "placeholders": [],
    "assumptions": [],
    "emptyEnv": []
  },
  "additions": [],
  "coverage": {
    "found": 2,
    "fixed": 2,
    "deferred": 0,
    "items": []
  },
  "concerns": [
    "crates/classic-ext4 — commit не атомарен при сбое записи: вызывать только на рабочей копии",
    "crates/classic-ext4 — файл до 4 extent'ов, каталог растёт до глубины 1",
    "Windows — unit-тесты не запускаются без повышения прав, идут только в Linux",
    "сборка — приложения не запускались вживую: нужен UAC на Windows, дисплей на Linux",
    "src-tauri/src/commands/custom_image.rs:236 — Armbian-имя с Wukongpi даёт Allowed без probe, T03 должен определять только через probe_image",
    "src/components/layout/HomePage.tsx:250, src/App.tsx:231 — кэшированный образ без supports_autoconfig, настройки скроются после снятия флага",
    "src/hooks/useConnectivity.ts:37 — force offline читается, переключателя нет",
    "src-tauri/src/images/mod.rs:120 — set_var без восстановления",
    "src/config/wukong.ts:6 — двойник каталога не сверяется тестом",
    "src/locales/*.json — в 18 языках осталось по 23 Armbian: удалить переписанные ключи, чтобы сработал откат на en",
    ".github/workflows/maintenance-release.yml:775 — шаг latest.json и подпись updater остались",
    "src-tauri/src/autoconfig.rs:5 — ссылка docs.armbian.com",
    "история 1, 2, 3 — сценарий не проверен в собранном приложении",
    "src-tauri/device/firstboot.sh:313,343 — при сбое создания post-network файл настроек остаётся",
    "src-tauri/src/utils/path.rs:214, autoconfig.rs:345 — на Windows копия упавшего процесса живёт 12 часов вопреки истории 37",
    "src-tauri/device/firstboot.sh:337,342 — пользователь без пароля и без ключей оставляет 1234 и автологин, нужен D## или запрет",
    "src-tauri/src/autoconfig.rs:59 — render_qdl_preset расходится с interfaces §3, нужен D##",
    "src-tauri/src/provision/bundle.rs:172, firstboot.sh:148 — отвергается только root, nobody получает sudo",
    "src-tauri/src/autoconfig.rs:451 — нет теста отмены между копией и вставкой",
    "src-tauri/src/provision/image.rs:198 — режим 0600 не проверяется на записанном образе; сценарии скриптов не закоммичены",
    "src-tauri/src/autoconfig.rs:28 — const _ держит мёртвую константу",
    "src/config/profileEditorModel.ts — зарезервированные логины только в TS, Rust отклоняет только root",
    "src/locales/*.json — в 18 языках устаревшие тексты laterLine, rootHint",
    "T04 — реальные команды Tauri и settings.json не проверены: только подмена",
    "src/config/profileEditorModel.ts:277 — сменив SSID, нельзя сделать сеть открытой",
    "src/config/autoconfig.ts:159 — комментарий про отказ строк с [ устарел",
    "src/config/profileEditorModel.ts:132 — страна Wi-Fi скрыта в мастере, en-интерфейс молча даёт US",
    "src/config/profileEditorModel.devtest.ts — нет тестов связки user+access; просмотр settings.json закрыть в T06",
    "src/config/autoconfig.ts — строка '# ssh-rsa AAAA' считается ключом",
    "src-tauri/device/post-network.sh — свой скрипт выполняется не более одного раза: обрыв питания во время него не повторяет его",
    "src-tauri/src/commands/custom_image.rs — const _ держат мёртвые AUTOCONFIG_CUSTOM_EXTENSIONS и RAW_IMAGE_EXTENSION",
    "src-tauri/device/post-network.sh — wipe_everything не проверяет результат: при сбое shred и rm unit снимается и firstboot.env остаётся без повтора; disable только после проверки, что каталог удалён",
    "src-tauri/device/post-network.sh — обрыв после mv в .wiping: ConditionPathExists ложно, остаток не дочищается, ветка [[ -e self ]] мёртвая",
    "src-tauri/device/post-network.sh — сбой install в stage_user_script не логируется, скрипт молча теряется",
    "src-tauri/device — шелл-тесты сценариев не в коммите",
    "src-tauri/device/post-network.sh — G02 не работает: авто-установка avahi падает всегда, имя.local не появится. apt-get update выходит с 100 из-за убранного bullseye-backports, avahi в security для armhf отдаёт 404. Проверенный обход T06: apt со своим sourcelist без backports и security. Установка в QEMU заняла ~15 мин при APT_TIMEOUT_SECS=300, таймаут пересмотреть",
    "T06 — лог /var/log/wukong-firstboot.log пропал после жёсткой остановки QEMU (проверить ramlog/zram и журнал на диске): G07 обещает лог при сбое Wi-Fi",
    "T06 — sudo reboot в QEMU не поднялся за 8 мин, причина не выяснена: QEMU или наша настройка? Первая и вторая холодные загрузки работают",
    "T06 — уникальность ключей хоста SSH на карту не проверена, только наличие",
    "docs/hardware-checklist.md — путь %APPDATA%\\io.github.kostec.wukongimager\\settings.json выведен из identifier, не проверен на запущенной программе",
    "F1 — что будет с dpkg при срабатывании предела 3600 с, не проверялось",
    "F1 — постоянный drop-in armbian-ramlog остаётся в системе и действует со следующей загрузки; на первой ramlog может быть убит таймаутом",
    "F1 — неудачное затирание проверено только на заглушках"
  ],
  "blind": {
    "checked": 36,
    "matched": 32,
    "mismatches": [
      "G02 — имя.local работает только при интернете на первой загрузке: avahi ставится через apt",
      "G04 — после подключения источника производители слиты в одну плитку «Other — 2 boards»; URL и пример nginx не запускались",
      "R16 — сборка под Linux в WSL невозможна (нет webkit2gtk и Rust), собирается только в Docker",
      "R26i — интерфейс разрешает открытую сеть Wi-Fi без пароля при договорённости «только WPA2»"
    ]
  },
  "beats": [
    "2026-10-30T09:49:18+03:00",
    "2026-10-30T09:49:38+03:00",
    "2026-10-30T09:50:02+03:00",
    "2026-10-30T09:50:05+03:00",
    "2026-10-30T09:54:46+03:00",
    "2026-10-30T10:06:51+03:00",
    "2026-10-30T10:23:53+03:00",
    "2026-10-30T10:31:27+03:00",
    "2026-10-30T10:32:03+03:00",
    "2026-10-30T10:32:05+03:00",
    "2026-10-30T10:52:27+03:00",
    "2026-10-30T10:52:31+03:00",
    "2026-10-30T10:54:48+03:00",
    "2026-10-30T10:54:49+03:00",
    "2026-10-30T10:54:50+03:00",
    "2026-10-30T10:54:59+03:00",
    "2026-10-30T10:55:00+03:00",
    "2026-10-30T10:55:03+03:00",
    "2026-10-30T10:55:30+03:00",
    "2026-10-30T10:55:31+03:00",
    "2026-10-30T11:44:23+03:00",
    "2026-10-30T11:44:26+03:00",
    "2026-10-30T12:53:43+03:00",
    "2026-10-30T12:53:48+03:00",
    "2026-10-30T12:53:49+03:00",
    "2026-10-30T12:53:50+03:00",
    "2026-10-30T12:58:36+03:00",
    "2026-10-30T12:58:40+03:00",
    "2026-10-30T14:02:20+03:00",
    "2026-10-30T14:03:09+03:00",
    "2026-10-30T14:38:49+03:00",
    "2026-10-30T14:38:53+03:00",
    "2026-10-30T14:38:54+03:00",
    "2026-10-30T14:38:55+03:00",
    "2026-10-30T14:38:56+03:00",
    "2026-10-30T14:45:30+03:00",
    "2026-10-30T15:01:15+03:00",
    "2026-10-30T15:01:16+03:00",
    "2026-10-30T15:01:17+03:00",
    "2026-10-30T15:06:58+03:00",
    "2026-10-30T15:07:05+03:00",
    "2026-10-30T15:10:52+03:00",
    "2026-10-30T15:11:58+03:00",
    "2026-10-30T15:11:59+03:00",
    "2026-10-30T15:46:43+03:00",
    "2026-10-30T15:46:45+03:00",
    "2026-10-30T15:46:46+03:00",
    "2026-10-30T15:51:32+03:00",
    "2026-10-30T15:51:33+03:00",
    "2026-10-30T17:20:11+03:00",
    "2026-10-30T17:20:13+03:00",
    "2026-10-30T17:20:14+03:00",
    "2026-10-30T17:20:15+03:00",
    "2026-10-30T17:28:45+03:00",
    "2026-10-30T17:28:46+03:00",
    "2026-10-30T17:28:54+03:00",
    "2026-10-30T19:19:46+03:00",
    "2026-10-30T22:08:24+03:00",
    "2026-10-30T22:08:25+03:00",
    "2026-10-30T22:08:26+03:00",
    "2026-10-30T22:13:12+03:00",
    "2026-10-30T22:13:13+03:00",
    "2026-10-30T22:13:14+03:00",
    "2026-10-31T00:12:08+03:00",
    "2026-10-31T08:57:38+03:00",
    "2026-10-31T09:53:34+03:00",
    "2026-10-31T10:01:40+03:00",
    "2026-10-31T10:02:43+03:00"
  ],
  "rev": 75,
  "waiting": null,
  "waits": [
    {
      "kind": "stop",
      "from": "2026-10-30T09:54:46+03:00",
      "to": "2026-10-30T10:06:51+03:00"
    },
    {
      "kind": "briefing",
      "from": "2026-10-30T10:06:51+03:00",
      "to": "2026-10-30T10:32:03+03:00"
    },
    {
      "kind": "stop",
      "from": "2026-10-31T00:12:08+03:00",
      "to": "2026-10-31T08:57:38+03:00"
    }
  ],
  "log": [
    {
      "t": "2026-10-30T09:49:18+03:00",
      "k": "init"
    },
    {
      "t": "2026-10-30T09:49:38+03:00",
      "k": "stage",
      "id": "manifest",
      "act": "start"
    },
    {
      "t": "2026-10-30T09:50:05+03:00",
      "k": "stage",
      "id": "briefing",
      "act": "start"
    },
    {
      "t": "2026-10-30T09:54:46+03:00",
      "k": "ask",
      "kind": "stop",
      "note": "Токен GitHub открытым текстом в OS/notes.txt — нужен отзыв; плюс вопросы брифинга"
    },
    {
      "t": "2026-10-30T10:06:51+03:00",
      "k": "answered",
      "kind": "stop"
    },
    {
      "t": "2026-10-30T10:06:51+03:00",
      "k": "ask",
      "kind": "briefing",
      "note": "Раунд 2: каталог, ISO, размещение форка и установка Rust, проверка на плате"
    },
    {
      "t": "2026-10-30T10:23:53+03:00",
      "k": "ask",
      "kind": "briefing",
      "note": "Раунд 3: хранение секретов, сбой Wi-Fi, частичные настройки, название и обновления"
    },
    {
      "t": "2026-10-30T10:31:27+03:00",
      "k": "ask",
      "kind": "briefing",
      "note": "Уточнение: как хранить пароли"
    },
    {
      "t": "2026-10-30T10:32:03+03:00",
      "k": "answered",
      "kind": "briefing"
    },
    {
      "t": "2026-10-30T10:32:05+03:00",
      "k": "gate",
      "g": "G1",
      "ok": false,
      "open": 33,
      "total": 36
    },
    {
      "t": "2026-10-30T10:32:05+03:00",
      "k": "stage",
      "id": "spec",
      "act": "start"
    },
    {
      "t": "2026-10-30T10:52:27+03:00",
      "k": "gate",
      "g": "G2",
      "ok": true,
      "found": 2,
      "fixed": 2,
      "deferred": 0
    },
    {
      "t": "2026-10-30T10:52:31+03:00",
      "k": "stage",
      "id": "plan",
      "act": "start"
    },
    {
      "t": "2026-10-30T10:54:48+03:00",
      "k": "tier",
      "v": "T2"
    },
    {
      "t": "2026-10-30T10:54:49+03:00",
      "k": "tickets",
      "n": 6
    },
    {
      "t": "2026-10-30T10:54:50+03:00",
      "k": "gate",
      "g": "G3",
      "ok": false,
      "problems": 2
    },
    {
      "t": "2026-10-30T10:54:59+03:00",
      "k": "tickets",
      "n": 6
    },
    {
      "t": "2026-10-30T10:55:00+03:00",
      "k": "gate",
      "g": "G3",
      "ok": true,
      "problems": 0
    },
    {
      "t": "2026-10-30T10:55:30+03:00",
      "k": "stage",
      "id": "build",
      "act": "start"
    },
    {
      "t": "2026-10-30T10:55:31+03:00",
      "k": "ticket",
      "ids": [
        "01",
        "02"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-30T11:44:23+03:00",
      "k": "ticket",
      "ids": [
        "02"
      ],
      "act": "done",
      "tests": {
        "passed": 27,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T11:44:26+03:00",
      "k": "add",
      "key": "concerns",
      "n": 2
    },
    {
      "t": "2026-10-30T12:53:43+03:00",
      "k": "ticket",
      "ids": [
        "01"
      ],
      "act": "done",
      "tests": {
        "passed": 229,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T12:53:48+03:00",
      "k": "ticket",
      "ids": [
        "01"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-30T12:53:49+03:00",
      "k": "add",
      "key": "concerns",
      "n": 2
    },
    {
      "t": "2026-10-30T12:53:50+03:00",
      "k": "add",
      "key": "report",
      "n": 1
    },
    {
      "t": "2026-10-30T12:53:50+03:00",
      "k": "ticket",
      "ids": [
        "03",
        "05"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-30T12:58:36+03:00",
      "k": "ticket",
      "ids": [
        "01"
      ],
      "act": "repair",
      "note": "AppImage из linux-docker.sh проходит appimage-unbundle-wayland.sh; тест отказа Url/Folder убирает T05"
    },
    {
      "t": "2026-10-30T12:58:40+03:00",
      "k": "add",
      "key": "concerns",
      "n": 9
    },
    {
      "t": "2026-10-30T14:02:20+03:00",
      "k": "ticket",
      "ids": [
        "01"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-30T14:03:09+03:00",
      "k": "ticket",
      "ids": [
        "01"
      ],
      "act": "done",
      "tests": {
        "passed": 229,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T14:38:49+03:00",
      "k": "ticket",
      "ids": [
        "05"
      ],
      "act": "done",
      "tests": {
        "passed": 294,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T14:38:53+03:00",
      "k": "ticket",
      "ids": [
        "03"
      ],
      "act": "done",
      "tests": {
        "passed": 294,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T14:38:54+03:00",
      "k": "ticket",
      "ids": [
        "03"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-30T14:38:55+03:00",
      "k": "ticket",
      "ids": [
        "03"
      ],
      "act": "repair",
      "note": "firstboot.conf переименовать в firstboot.env; .iso через probe_image"
    },
    {
      "t": "2026-10-30T14:38:56+03:00",
      "k": "ticket",
      "ids": [
        "04"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-30T14:45:30+03:00",
      "k": "add",
      "key": "concerns",
      "n": 8
    },
    {
      "t": "2026-10-30T15:01:15+03:00",
      "k": "ticket",
      "ids": [
        "04"
      ],
      "act": "done",
      "tests": {
        "passed": 20,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T15:01:16+03:00",
      "k": "ticket",
      "ids": [
        "04"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-30T15:01:17+03:00",
      "k": "add",
      "key": "concerns",
      "n": 3
    },
    {
      "t": "2026-10-30T15:06:58+03:00",
      "k": "ticket",
      "ids": [
        "04"
      ],
      "act": "repair",
      "note": "SSH без пользователя и с закрытым root не сохраняется; ключом считается только строка с типом, PEM отказывается"
    },
    {
      "t": "2026-10-30T15:07:05+03:00",
      "k": "add",
      "key": "concerns",
      "n": 4
    },
    {
      "t": "2026-10-30T15:10:52+03:00",
      "k": "ticket",
      "ids": [
        "04"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-30T15:11:58+03:00",
      "k": "ticket",
      "ids": [
        "04"
      ],
      "act": "done",
      "tests": {
        "passed": 25,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T15:11:59+03:00",
      "k": "add",
      "key": "concerns",
      "n": 1
    },
    {
      "t": "2026-10-30T15:46:43+03:00",
      "k": "ticket",
      "ids": [
        "03"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-30T15:46:45+03:00",
      "k": "ticket",
      "ids": [
        "06"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-30T15:46:46+03:00",
      "k": "add",
      "key": "concerns",
      "n": 2
    },
    {
      "t": "2026-10-30T15:51:32+03:00",
      "k": "ticket",
      "ids": [
        "03"
      ],
      "act": "done",
      "tests": {
        "passed": 301,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T15:51:33+03:00",
      "k": "add",
      "key": "concerns",
      "n": 4
    },
    {
      "t": "2026-10-30T17:20:11+03:00",
      "k": "ticket",
      "ids": [
        "06"
      ],
      "act": "done",
      "tests": {
        "passed": 46,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T17:20:13+03:00",
      "k": "tests",
      "passed": 301,
      "failed": 0
    },
    {
      "t": "2026-10-30T17:20:14+03:00",
      "k": "add",
      "key": "concerns",
      "n": 5
    },
    {
      "t": "2026-10-30T17:20:15+03:00",
      "k": "stage",
      "id": "review",
      "act": "start"
    },
    {
      "t": "2026-10-30T17:28:45+03:00",
      "k": "add",
      "key": "report",
      "n": 14
    },
    {
      "t": "2026-10-30T17:28:46+03:00",
      "k": "tickets",
      "n": 7
    },
    {
      "t": "2026-10-30T17:28:54+03:00",
      "k": "ticket",
      "ids": [
        "F1"
      ],
      "act": "start"
    },
    {
      "t": "2026-10-30T19:19:46+03:00",
      "k": "ticket",
      "ids": [
        "F1"
      ],
      "act": "repair",
      "note": "e2e: после kill -9 и холодной загрузки нет /var/log/wukong-firstboot.log, own script count пуст"
    },
    {
      "t": "2026-10-30T22:08:24+03:00",
      "k": "ticket",
      "ids": [
        "F1"
      ],
      "act": "done",
      "tests": {
        "passed": 301,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T22:08:25+03:00",
      "k": "ticket",
      "ids": [
        "F1"
      ],
      "act": "review"
    },
    {
      "t": "2026-10-30T22:08:26+03:00",
      "k": "add",
      "key": "concerns",
      "n": 3
    },
    {
      "t": "2026-10-30T22:13:12+03:00",
      "k": "ticket",
      "ids": [
        "F1"
      ],
      "act": "done",
      "tests": {
        "passed": 301,
        "failed": 0
      }
    },
    {
      "t": "2026-10-30T22:13:13+03:00",
      "k": "add",
      "key": "report",
      "n": 6
    },
    {
      "t": "2026-10-30T22:13:14+03:00",
      "k": "stage",
      "id": "final",
      "act": "start"
    },
    {
      "t": "2026-10-31T00:12:08+03:00",
      "k": "ask",
      "kind": "stop",
      "note": "Пользователь остановил прогон на слепой приёмке"
    },
    {
      "t": "2026-10-31T08:57:38+03:00",
      "k": "answered",
      "kind": "stop"
    },
    {
      "t": "2026-10-31T09:53:34+03:00",
      "k": "gate",
      "g": "G4",
      "ok": false,
      "checked": 36,
      "matched": 32
    },
    {
      "t": "2026-10-31T10:01:40+03:00",
      "k": "tests",
      "passed": 301,
      "failed": 0
    },
    {
      "t": "2026-10-31T10:02:43+03:00",
      "k": "finish",
      "result": "Wukong Imager: запись .img/.iso на SD с первичной настройкой Wukong Pi Zero, GUI под Windows и Linux"
    }
  ],
  "reqList": [
    {
      "id": "R01",
      "text": "«аналог Raspberry Pi Imager, но для Wukong Pi»",
      "status": "done"
    },
    {
      "id": "R02",
      "text": "«плата Wukong PI Zero»",
      "status": "done"
    },
    {
      "id": "R03",
      "text": "«загрузка образа только вручную zip»",
      "status": "dropped",
      "why": "пользователь: «Загрузка из zip не является приоритетом, как и CLI, от них можно отказаться»"
    },
    {
      "id": "R04",
      "text": "«[загрузка образа только вручную] iso файлом»",
      "status": "done"
    },
    {
      "id": "R05",
      "text": "«zip файл формируется в проекте @wukong-build, например в Github Actions»",
      "status": "dropped",
      "why": "пользователь: «Загрузка из zip не является приоритетом… от них можно отказаться»"
    },
    {
      "id": "R06",
      "text": "«первоначальную настройку… логин/пароль пользователя»",
      "status": "done"
    },
    {
      "id": "R07",
      "text": "«первоначальную настройку… подключение к wifi»",
      "status": "done"
    },
    {
      "id": "R08",
      "text": "«первоначальную настройку… настройка доступа по ssh»",
      "status": "done"
    },
    {
      "id": "R09",
      "text": "«Придумай ещё несколько полезных пунктов - обсудим»",
      "status": "done"
    },
    {
      "id": "R10",
      "text": "«После всех настроек запускается прошивка»",
      "status": "done"
    },
    {
      "id": "R11",
      "text": "«После запуска с прошитой SD карты устройство должно запуститься с указанными настройками»",
      "status": "done"
    },
    {
      "id": "R12",
      "text": "«если был указан Wifi, то устройство должно к нему подключиться»",
      "status": "done"
    },
    {
      "id": "R13",
      "text": "«если указаны настройки SSH, то SSH должен работать»",
      "status": "done"
    },
    {
      "id": "R14",
      "text": "«Можно пользоваться уже существующими библиотеками, которые покрывают часть задачи или задачу целиком»",
      "status": "done"
    },
    {
      "id": "R15",
      "text": "«кроссплатформенной… запуск… из под Windows»",
      "status": "done"
    },
    {
      "id": "R16",
      "text": "«кроссплатформенной… запуск… из под Linux»",
      "status": "done"
    },
    {
      "id": "R17",
      "text": "«работы из CLI»",
      "status": "dropped",
      "why": "пользователь: «Загрузка из zip не является приоритетом, как и CLI, от них можно отказаться»"
    },
    {
      "id": "R18",
      "text": "«[работы из] GUI»",
      "status": "done"
    },
    {
      "id": "R19",
      "text": "«основным ориентиром является Windows»",
      "status": "done"
    },
    {
      "id": "R20",
      "text": "«Возможно мне уже подходит Armbian Imager… проанализируй насколько он подходит»",
      "status": "done"
    },
    {
      "id": "R21",
      "text": "«можно сделать его форк и добавить поддержку нужной платы»",
      "status": "done"
    },
    {
      "id": "R22i",
      "text": "(подразумевается) прошивка не должна затереть не тот диск: выбор только съёмных SD/USB и подтверждение",
      "status": "done"
    },
    {
      "id": "R23i",
      "text": "(подразумевается) записанный образ проверяется, иначе «прошилось» ничего не значит",
      "status": "done"
    },
    {
      "id": "R24i",
      "text": "(подразумевается) без настроек образ записывается как есть и поведение устройства не меняется",
      "status": "done"
    },
    {
      "id": "R25i",
      "text": "(подразумевается) образ должен уметь применять настройки: нужен механизм на стороне wukong-build или правка образа при записи",
      "status": "done"
    },
    {
      "id": "R26i",
      "text": "(подразумевается) пароли и ключи Wi-Fi не остаются на карте открытым текстом после первой загрузки",
      "status": "done"
    },
    {
      "id": "G01",
      "text": "«при первом запуске все настройки уже были применены»",
      "status": "done"
    },
    {
      "id": "G02",
      "text": "«Твой набор устраивает»: hostname и wukong.local; часовой пояс, локаль, раскладка; страна Wi-Fi; статический IP или DHCP; SSH по паролю и/или ключу, запрет входа под root; пароль root; профили настроек, импорт ключа с GitHub; пропуск масте…",
      "status": "done"
    },
    {
      "id": "G03",
      "text": "«каталог… пока что его можно отключить и работать только с Wukong»",
      "status": "done"
    },
    {
      "id": "G04",
      "text": "«рассмотрел возможность подключения кастомного конфига, который даст доступ к другим вендорам по api (например, локальный сервер в Docker)»",
      "status": "done"
    },
    {
      "id": "G05",
      "text": "«или использовал локальное хранилище на диске пользователя»",
      "status": "done"
    },
    {
      "id": "G06",
      "text": "«свою проверку сделаю после прогона, оставь для меня чеклист»",
      "status": "done"
    },
    {
      "id": "G07",
      "text": "«[при сбое Wi-Fi] 2. ОК»: Ethernet не отключается, остальные настройки применяются, лог в /var/log/wukong-firstboot.log, вход через UART работает, перезагрузок нет",
      "status": "done"
    },
    {
      "id": "G08",
      "text": "«[частичные настройки] 3. ОК»: применяется только заполненное; без пользователя мастер первого входа остаётся; при включённых Wi-Fi или SSH предупреждение о root/1234",
      "status": "done"
    },
    {
      "id": "G09",
      "text": "«[название] 4. Ок»: Wukong Imager, нейтральный значок, логотипы Armbian убраны",
      "status": "done"
    },
    {
      "id": "G10",
      "text": "«[обновления] 4. Ок»: автообновление и отправка логов на серверы Armbian отключены; код других плат и macOS не трогается и не проверяется; установщик без подписи",
      "status": "done"
    }
  ],
  "gates": {
    "G1": {
      "ok": false,
      "at": "2026-10-30T10:32:05+03:00",
      "open": 33,
      "total": 36
    },
    "G2": {
      "ok": true,
      "at": "2026-10-30T10:52:27+03:00",
      "found": 2,
      "fixed": 2,
      "deferred": 0
    },
    "G3": {
      "ok": true,
      "at": "2026-10-30T10:55:00+03:00",
      "problems": 0
    },
    "G4": {
      "ok": false,
      "at": "2026-10-31T09:53:34+03:00",
      "checked": 36,
      "matched": 32
    }
  },
  "result": "Wukong Imager: запись .img/.iso на SD с первичной настройкой Wukong Pi Zero, GUI под Windows и Linux",
  "report": [
    "Собранные приложения в прогоне не запускались вживую: запись на карту проверяется по чек-листу",
    "Ключи хоста SSH: по коду каждая карта получает свои, но на двух картах не сравнивалось (строка в чек-листе)",
    "Безопасность: если служба второго этапа не создалась (диск заполнен или только на чтение), файл с хешами и ключом Wi-Fi останется на карте",
    "Безопасность: на Windows после падения программы копия образа с хешами и ключом Wi-Fi лежит до запуска спустя 12 часов. Открытых паролей в ней нет",
    "Безопасность: если в каталоге заменить файл и sha256, оставив имя, запишется старый образ из кэша без сверки",
    "Безопасность: зарезервированные логины проверяет только форма; после ручной правки settings.json пользователь nobody получит sudo",
    "Строка «# ssh-rsa AAAA» считается ключом, поэтому запрет «SSH без пароля и без ключей» можно обойти и остаться без входа",
    "Форма: сеть с паролем нельзя сделать открытой. Страна Wi-Fi спрятана, в английском интерфейсе молча ставится US, сети на каналах 12-13 могут быть не видны",
    "Без заданного имени устройства avahi не ставится, но подсказка покажет адрес wukongpi.local",
    "Свой скрипт выполняется не больше одного раза: при обрыве питания во время него он не повторится",
    "Не проверено прогоном: SSH выключен, открытая сеть, статический IP, повтор после обрыва питания, отмена между копией и вставкой, настоящие команды Tauri и settings.json, сценарии в собранном приложении",
    "Шелл-сценарии скриптов устройства в репозиторий не вошли",
    "Проверка «без лишней перезагрузки» в e2e не может покраснеть: журнал volatile показывает одну загрузку",
    "Проба образа не смотрит, примет ли /etc/systemd/system новую запись: на чужом образе с индексом отказ придёт только при записи",
    "Мелочи: двойник каталога src/config/wukong.ts без теста сверки; путь settings.json в чек-листе не проверен; render_qdl_preset расходится с interfaces; maintenance-release.yml ещё генерирует latest.json",
    "Если установка avahi упрётся в предел в 1 час, dpkg может быть прерван; следующая загрузка попробует допоставить, но такой случай не проверялся",
    "Причина сбоя Wi-Fi от настоящего адаптера проверена только на заглушках: у QEMU адаптера нет. Проверяется на плате, §12 чек-листа",
    "Если файл настроек уже удалён, а служба-уборщик не дочистила остаток (сбой или обрыв питания в короткий миг), повтора не будет, хотя документация его обещает. Секретов там уже нет",
    "В чек-листе, §11, возможно неверно сказано, где лежит журнал на карте в чужом Linux: сверить на образе",
    "Падение проверочного device-test на середине не считается ошибкой e2e: часть проверок может молча выпасть",
    "Проверка CI maintenance-pr-check, шаг сверки локалей, красная: было 1260 ошибок, стало 1962. Переводы с откатом на en ей не соответствуют, проверку нужно переписать"
  ]
}
