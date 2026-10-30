window.STATE =
{
  "version": 2,
  "slug": "wukong-imager",
  "dir": "2026-10-30-wukong-imager--wip",
  "title": "Wukong Imager — прошивальщик SD с первичной настройкой",
  "mode": "interview",
  "depth": "deep",
  "tier": "T2",
  "briefFile": "2026-10-30-brief.md",
  "memoryFile": "AGENTS.md",
  "memoryOwner": "autopilot",
  "skillDir": "C:/Users/work/.agents/skills/autopilot",
  "baseCommit": null,
  "startedAt": "2026-10-30T09:49:18+03:00",
  "updatedAt": "2026-10-30T10:55:00+03:00",
  "finishedAt": null,
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
      "status": "active",
      "startedAt": "2026-10-30T10:52:31+03:00"
    },
    {
      "id": "build",
      "status": "pending"
    },
    {
      "id": "review",
      "status": "pending"
    },
    {
      "id": "final",
      "status": "pending"
    }
  ],
  "requirements": {
    "total": 36,
    "done": 0,
    "inTicket": 33,
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
      "status": "pending",
      "retries": 0,
      "repairs": 0
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
      "status": "pending",
      "retries": 0,
      "repairs": 0
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
        "src-tauri/src/ssh_keys.rs"
      ],
      "review": "да — пароли и секреты",
      "model": "сильная",
      "status": "pending",
      "retries": 0,
      "repairs": 0
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
        "src/types/index.ts"
      ],
      "review": "да — пароли и секреты",
      "model": "сильная",
      "status": "pending",
      "retries": 0,
      "repairs": 0
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
        "src/components/layout/OsPanel.tsx",
        "docs/catalog/"
      ],
      "review": "нет",
      "model": "обычная",
      "status": "pending",
      "retries": 0,
      "repairs": 0
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
      "status": "pending",
      "retries": 0,
      "repairs": 0
    }
  ],
  "tests": null,
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
  "concerns": [],
  "blind": null,
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
    "2026-10-30T10:55:00+03:00"
  ],
  "rev": 20,
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
    }
  ],
  "reqList": [
    {
      "id": "R01",
      "text": "«аналог Raspberry Pi Imager, но для Wukong Pi»",
      "status": "in-ticket"
    },
    {
      "id": "R02",
      "text": "«плата Wukong PI Zero»",
      "status": "in-ticket"
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
      "status": "in-ticket"
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
      "status": "in-ticket"
    },
    {
      "id": "R07",
      "text": "«первоначальную настройку… подключение к wifi»",
      "status": "in-ticket"
    },
    {
      "id": "R08",
      "text": "«первоначальную настройку… настройка доступа по ssh»",
      "status": "in-ticket"
    },
    {
      "id": "R09",
      "text": "«Придумай ещё несколько полезных пунктов - обсудим»",
      "status": "in-ticket"
    },
    {
      "id": "R10",
      "text": "«После всех настроек запускается прошивка»",
      "status": "in-ticket"
    },
    {
      "id": "R11",
      "text": "«После запуска с прошитой SD карты устройство должно запуститься с указанными настройками»",
      "status": "in-ticket"
    },
    {
      "id": "R12",
      "text": "«если был указан Wifi, то устройство должно к нему подключиться»",
      "status": "in-ticket"
    },
    {
      "id": "R13",
      "text": "«если указаны настройки SSH, то SSH должен работать»",
      "status": "in-ticket"
    },
    {
      "id": "R14",
      "text": "«Можно пользоваться уже существующими библиотеками, которые покрывают часть задачи или задачу целиком»",
      "status": "in-ticket"
    },
    {
      "id": "R15",
      "text": "«кроссплатформенной… запуск… из под Windows»",
      "status": "in-ticket"
    },
    {
      "id": "R16",
      "text": "«кроссплатформенной… запуск… из под Linux»",
      "status": "in-ticket"
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
      "status": "in-ticket"
    },
    {
      "id": "R19",
      "text": "«основным ориентиром является Windows»",
      "status": "in-ticket"
    },
    {
      "id": "R20",
      "text": "«Возможно мне уже подходит Armbian Imager… проанализируй насколько он подходит»",
      "status": "in-ticket"
    },
    {
      "id": "R21",
      "text": "«можно сделать его форк и добавить поддержку нужной платы»",
      "status": "in-ticket"
    },
    {
      "id": "R22i",
      "text": "(подразумевается) прошивка не должна затереть не тот диск: выбор только съёмных SD/USB и подтверждение",
      "status": "in-ticket"
    },
    {
      "id": "R23i",
      "text": "(подразумевается) записанный образ проверяется, иначе «прошилось» ничего не значит",
      "status": "in-ticket"
    },
    {
      "id": "R24i",
      "text": "(подразумевается) без настроек образ записывается как есть и поведение устройства не меняется",
      "status": "in-ticket"
    },
    {
      "id": "R25i",
      "text": "(подразумевается) образ должен уметь применять настройки: нужен механизм на стороне wukong-build или правка образа при записи",
      "status": "in-ticket"
    },
    {
      "id": "R26i",
      "text": "(подразумевается) пароли и ключи Wi-Fi не остаются на карте открытым текстом после первой загрузки",
      "status": "in-ticket"
    },
    {
      "id": "G01",
      "text": "«при первом запуске все настройки уже были применены»",
      "status": "in-ticket"
    },
    {
      "id": "G02",
      "text": "«Твой набор устраивает»: hostname и wukong.local; часовой пояс, локаль, раскладка; страна Wi-Fi; статический IP или DHCP; SSH по паролю и/или ключу, запрет входа под root; пароль root; профили настроек, импорт ключа с GitHub; пропуск масте…",
      "status": "in-ticket"
    },
    {
      "id": "G03",
      "text": "«каталог… пока что его можно отключить и работать только с Wukong»",
      "status": "in-ticket"
    },
    {
      "id": "G04",
      "text": "«рассмотрел возможность подключения кастомного конфига, который даст доступ к другим вендорам по api (например, локальный сервер в Docker)»",
      "status": "in-ticket"
    },
    {
      "id": "G05",
      "text": "«или использовал локальное хранилище на диске пользователя»",
      "status": "in-ticket"
    },
    {
      "id": "G06",
      "text": "«свою проверку сделаю после прогона, оставь для меня чеклист»",
      "status": "in-ticket"
    },
    {
      "id": "G07",
      "text": "«[при сбое Wi-Fi] 2. ОК»: Ethernet не отключается, остальные настройки применяются, лог в /var/log/wukong-firstboot.log, вход через UART работает, перезагрузок нет",
      "status": "in-ticket"
    },
    {
      "id": "G08",
      "text": "«[частичные настройки] 3. ОК»: применяется только заполненное; без пользователя мастер первого входа остаётся; при включённых Wi-Fi или SSH предупреждение о root/1234",
      "status": "in-ticket"
    },
    {
      "id": "G09",
      "text": "«[название] 4. Ок»: Wukong Imager, нейтральный значок, логотипы Armbian убраны",
      "status": "in-ticket"
    },
    {
      "id": "G10",
      "text": "«[обновления] 4. Ок»: автообновление и отправка логов на серверы Armbian отключены; код других плат и macOS не трогается и не проверяется; установщик без подписи",
      "status": "in-ticket"
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
    }
  },
  "result": null
}
