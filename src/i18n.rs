pub fn language(value: &str) -> String {
    if value != "system" {
        return value.into();
    }
    let locale = crate::platform::Desktop::system_language();
    if locale.starts_with("uk") {
        "uk"
    } else if locale.starts_with("ru") {
        "ru"
    } else {
        "en"
    }
    .into()
}
pub fn t<'a>(language: &str, key: &'a str) -> &'a str {
    let words = match key {
        "accounts" => ["Акаунти", "Accounts", "Аккаунты"],
        "manage" => ["Керування", "Manage", "Управление"],
        "settings" => ["Налаштування", "Settings", "Настройки"],
        "add" => ["Додати акаунт", "Add account", "Добавить аккаунт"],
        "register" => [
            "Зареєструвати поточний вхід",
            "Register current login",
            "Зарегистрировать текущий вход",
        ],
        "browser_hint" => [
            "Вхід відкриється у браузері",
            "Sign in through your browser",
            "Вход откроется в браузере",
        ],
        "cancel" => ["Скасувати", "Cancel", "Отмена"],
        "confirm" => ["Підтвердити", "Confirm", "Подтвердить"],
        "switch" => [
            "Перемикання акаунта",
            "Switch account",
            "Переключение аккаунта",
        ],
        "switch_hint" => [
            "Codex Desktop закриється та відкриється з вибраним акаунтом. Завершіть поточні завдання перед підтвердженням.",
            "Codex Desktop will close and reopen with the selected account. Finish current tasks before confirming.",
            "Codex Desktop закроется и откроется с выбранным аккаунтом. Завершите текущие задачи перед подтверждением.",
        ],
        "remove" => ["Видалити", "Remove", "Удалить"],
        "remove_hint" => [
            "Видалити локальну копію входу? Сам акаунт залишиться доступним у сервісі.",
            "Remove this saved local login? The service account will remain available.",
            "Удалить локальную копию входа? Сам аккаунт останется доступным в сервисе.",
        ],
        "empty" => [
            "Додайте акаунт або зареєструйте поточний вхід Codex.",
            "Add an account or register your current Codex login.",
            "Добавьте аккаунт или зарегистрируйте текущий вход Codex.",
        ],
        "weekly" => ["Тиждень", "Weekly", "Неделя"],
        "five_hour" => ["5 годин", "5 hours", "5 часов"],
        "resets" => ["Скидання", "Resets", "Сброс"],
        "credits" => [
            "Додаткові скидання",
            "Extra resets",
            "Дополнительные сбросы",
        ],
        "unknown" => ["Невідомо", "Unknown", "Неизвестно"],
        "subscription" => ["Підписка до", "Subscription until", "Подписка до"],
        "snapshot_hint" => [
            "Дата зі збереженого входу, не поточна перевірка оплати. Перевірено:",
            "Saved login snapshot, not a live billing check. Checked:",
            "Дата из сохранённого входа, не текущая проверка оплаты. Проверено:",
        ],
        "stale" => [
            "Збережені дані; оновлення не вдалося",
            "Saved data; refresh failed",
            "Сохранённые данные; обновление не удалось",
        ],
        "unavailable" => [
            "Ліміти недоступні",
            "Usage unavailable",
            "Лимиты недоступны",
        ],
        "general" => ["Загальні", "General", "Общие"],
        "startup" => [
            "Запускати з Windows",
            "Launch at Windows sign-in",
            "Запускать с Windows",
        ],
        "percentage" => [
            "Показувати відсоток у треї",
            "Show tray percentage",
            "Показывать процент в трее",
        ],
        "five_toggle" => [
            "Показувати ліміт за 5 годин",
            "Show 5-hour usage",
            "Показывать лимит за 5 часов",
        ],
        "tray_window" => [
            "Ліміт для іконки трея",
            "Tray usage window",
            "Лимит для значка трея",
        ],
        "language" => ["Мова", "Language", "Язык"],
        "system" => ["Системна", "System default", "Системный"],
        "experimental" => ["Експериментальні", "Experimental", "Экспериментальные"],
        "experiment" => [
            "Спостерігати за строком скидань",
            "Observe reset credit expiry",
            "Наблюдать за сроком сбросов",
        ],
        "experiment_hint" => [
            "Лише журналювання за 10, 5 та 1 хвилину до завершення строку. Скидання не витрачаються.",
            "Read-only logging 10, 5 and 1 minute before expiry. No reset credits are consumed.",
            "Только журналирование за 10, 5 и 1 минуту до истечения срока. Сбросы не расходуются.",
        ],
        "open_log" => [
            "Відкрити папку журналу",
            "Open log folder",
            "Открыть папку журнала",
        ],
        "updates" => ["Оновлення", "Updates", "Обновления"],
        "auto_update" => [
            "Автоматично перевіряти оновлення",
            "Automatically check updates",
            "Автоматически проверять обновления",
        ],
        "repository" => [
            "GitHub-репозиторій власних релізів",
            "GitHub repository for your releases",
            "GitHub-репозиторий своих релизов",
        ],
        "save" => ["Зберегти", "Save", "Сохранить"],
        "check" => [
            "Перевірити оновлення",
            "Check for updates",
            "Проверить обновления",
        ],
        "update_unconfigured" => [
            "Джерело оновлень не налаштоване",
            "Update source is not configured",
            "Источник обновлений не настроен",
        ],
        "checking" => ["Перевіряю…", "Checking…", "Проверяю…"],
        "update_available" => [
            "Доступна нова версія",
            "A new version is available",
            "Доступна новая версия",
        ],
        "up_to_date" => [
            "Установлено актуальну версію",
            "You are up to date",
            "Установлена актуальная версия",
        ],
        "update_failed" => [
            "Не вдалося перевірити оновлення",
            "Update check failed",
            "Не удалось проверить обновления",
        ],
        "download" => [
            "Відкрити сторінку релізу",
            "Open release page",
            "Открыть страницу релиза",
        ],
        "import" => [
            "Імпорт з оригіналу",
            "Import from original",
            "Импорт из оригинала",
        ],
        "choose_folder" => ["Вибрати папку…", "Choose folder…", "Выбрать папку…"],
        "preview" => ["Перегляд імпорту", "Import preview", "Просмотр импорта"],
        "duplicates" => [
            "Дублікатів буде пропущено",
            "Duplicates to skip",
            "Дубликатов будет пропущено",
        ],
        "import_hint" => [
            "Джерело та поточний вхід Codex не змінюються. Усі профілі перевіряються перед імпортом.",
            "Source data and current Codex login stay unchanged. All profiles are validated before import.",
            "Источник и текущий вход Codex не меняются. Все профили проверяются перед импортом.",
        ],
        "quit" => ["Вийти", "Quit", "Выйти"],
        "open" => ["Відкрити", "Open", "Открыть"],
        "error" => ["Помилка операції", "Operation failed", "Ошибка операции"],
        "dismiss" => ["Закрити повідомлення", "Dismiss", "Закрыть сообщение"],
        "login" => [
            "Очікую завершення входу…",
            "Waiting for browser sign-in…",
            "Ожидаю завершения входа…",
        ],
        "refresh" => ["Оновлюю ліміти…", "Refreshing usage…", "Обновляю лимиты…"],
        "working" => ["Виконую операцію…", "Working…", "Выполняю операцию…"],
        _ => return key,
    };
    words[match language {
        "uk" => 0,
        "ru" => 2,
        _ => 1,
    }]
}
