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
        "login_cancel_hint" => [
            "Якщо закрили браузер, натисніть «Скасувати» або Esc. Закриття браузера не завершує очікування входу.",
            "If you closed the browser, click Cancel or press Esc. Closing the browser does not cancel sign-in.",
            "Если закрыли браузер, нажмите «Отмена» или Esc. Закрытие браузера не отменяет ожидание входа.",
        ],
        "cancel" => ["Скасувати", "Cancel", "Отмена"],
        "confirm" => ["Підтвердити", "Confirm", "Подтвердить"],
        "switch" => [
            "Перемикання акаунта",
            "Switch account",
            "Переключение аккаунта",
        ],
        "switch_hint" => [
            "Codex Desktop закриється та відкриється знову. Спочатку завершіть або зупиніть завдання Desktop. Якщо з’явиться діалог виходу, підтвердьте його. Якщо Desktop не закриється штатно, перемикання зупиниться. Наявні CLI-сесії залишаться відкритими; нові використають вибраний акаунт.",
            "Codex Desktop will close and reopen. Finish or stop running Desktop tasks first. If Desktop asks to quit, complete its quit dialog. Switching stops if Desktop cannot exit normally. Existing CLI sessions stay open; new CLI sessions use the selected account.",
            "Codex Desktop закроется и откроется снова. Сначала завершите или остановите задачи Desktop. Если появится диалог выхода, подтвердите его. Если Desktop не закроется штатно, переключение остановится. Существующие CLI-сессии останутся открытыми; новые используют выбранный аккаунт.",
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
        "credits" => ["Скидань лишилось", "Resets left", "Осталось сбросов"],
        "unknown" => ["Невідомо", "Unknown", "Неизвестно"],
        "subscription" => ["Дійсний до", "Valid until", "Действует до"],
        "snapshot_hint" => [
            "Дата зі збереженого входу, не поточна перевірка оплати. Перевірено:",
            "Saved login snapshot, not a live billing check. Checked:",
            "Дата из сохранённого входа, не текущая проверка оплаты. Проверено:",
        ],
        "stale" => [
            "Збережені дані; потребують оновлення",
            "Saved data; refresh needed",
            "Сохранённые данные; требуется обновление",
        ],
        "usage_checked" => ["Ліміти отримано: ", "Usage fetched: ", "Лимиты получены: "],
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
        "left" => ["% лишилось", "% left", "% осталось"],
        "back" => ["Назад", "Back", "Назад"],
        "active" => ["Активний", "Active", "Активный"],
        "active_unconfirmed" => [
            "Не вдалося підтвердити активний акаунт",
            "Active account could not be confirmed",
            "Не удалось подтвердить активный аккаунт",
        ],
        "usage" => ["Ліміт", "Usage", "Лимит"],
        "version" => ["Версія", "Version", "Версия"],
        "switch_action" => ["Перемкнути", "Switch", "Переключить"],
        "switch_title" => ["Перемкнути на %@?", "Switch to %@?", "Переключить на %@?"],
        "remove_title" => ["Видалити %@?", "Remove %@?", "Удалить %@?"],
        _ => return key,
    };
    words[match language {
        "uk" => 0,
        "ru" => 2,
        _ => 1,
    }]
}
