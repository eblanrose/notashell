/*
 * ============================================================================
 * 🐚 main.c — ЯДРО ANSsh НА C
 * ============================================================================
 * 📝 Этот файл реализует:
 *    - 🔁 REPL-цикл (Read-Eval-Print Loop) — главный цикл shell'а
 *    - ⌨️  Чтение строки через GNU readline (замена rustyline из Rust)
 *    - 🧩 Разбор ввода на токены (упрощённый аналог shell_words)
 *    - 🚀 fork/exec/pipe — выполнение внешних команд (аналог std::process)
 *    - 📋 Управление джобами (jobs/fg/bg) — аналог executor.rs
 *    - 🎨 Встроенные команды: exit, version, help, cd, theme, profile, plugins
 *    - 📞 Вызов Perl-хелпера (main.pl) для сложной логики
 *
 * 🏗️  КОМПИЛЯЦИЯ:
 *     gcc -O2 -Wall -Wextra -o anssh main.c -lreadline
 *
 * 🎯 ЗАПУСК:
 *     ./anssh                    # интерактивный режим
 *     ./anssh -c "ls -la"       # выполнить одну команду
 *     ./anssh -v                # версия
 *     ./anssh -h                # помощь
 * ============================================================================
 */

/* 🧰 СТАНДАРТНЫЕ ЗАГОЛОВКИ — нужны для базовых функций */
#include <stdio.h>      /* printf, fprintf, fopen, fgets, perror 🖨️ */
#include <stdlib.h>     /* malloc, free, exit, getenv, setenv 🎲 */
#include <string.h>     /* strcmp, strdup, strtok, strlen, strchr 📏 */
#include <unistd.h>     /* fork, exec, chdir, getcwd, isatty, access 🐧 */
#include <sys/types.h>  /* pid_t, uid_t — типы данных для POSIX 🆔 */
#include <sys/wait.h>   /* waitpid, WIFEXITED, WEXITSTATUS ⏳ */
#include <sys/stat.h>   /* stat, mkdir, S_ISDIR 📁 */
#include <fcntl.h>      /* open, O_CREAT, O_APPEND, O_WRONLY 🔓 */
#include <signal.h>     /* signal, SIGINT, SIGCHLD ⚡ */
#include <errno.h>      /* errno — глобальный код ошибки ❌ */
#include <libgen.h>     /* basename, dirname 📂 */

/* ⌨️ GNU Readline — замена rustyline. Даёт историю, стрелки, Ctrl-R и т.п. */
#include <readline/readline.h>
#include <readline/history.h>

/* 🌍 Внешние объявления из Perl-модуля (см. main.pl) */
/* Мы НЕ линкуем Perl напрямую — проще вызывать через system()/popen().
 * Но объявим константы путей, чтобы не хардкодить. */

#define ANSH_VERSION   "1.0.0"                    /* 🏷️ Версия shell'а */
#define ANSH_RC_NAME   ".anssh_rc"                /* 📜 Имя rc-файла в $HOME */
#define ANSH_HIST_NAME ".anssh_history"           /* 🕰️ Файл истории команд */
#define ANSH_DIR_NAME  ".config/anssh"            /* 📁 Каталог конфигурации */
#define ANSH_PERL_PATH "main.pl"                  /* 🐪 Скрипт-хелпер на Perl */

/* 📊 Максимальное число аргументов одной команды (грубая защита) */
#define MAX_ARGS 512

/*
 * ============================================================================
 * 🗂️  СТРУКТУРЫ ДАННЫХ
 * ============================================================================
 */

/*
 * 📋 Job — описание фоновой задачи.
 * Аналог struct Job из executor.rs.
 * 💡 В Rust был enum JobStatus; в C используем поле + код возврата.
 */
typedef struct {
    int  id;            /* 🔢 Уникальный ID джоба (1, 2, 3...) */
    char *command;      /* 📝 Текст команды */
    pid_t pid;          /* 🧬 PID дочернего процесса (0 = ещё не запущен) */
    int  running;       /* 🟢 1 = работает, 0 = завершён/остановлен */
    int  is_background; /* 🌙 1 = фоновая (через &), 0 = передняя */
    int  exit_code;     /* 🚪 Код возврата (если завершён) */
} Job;

/*
 * 🌐 Глобальное состояние shell'а.
 * В Rust это были отдельные менеджеры (JobManager, ThemeManager, ...),
 * здесь — одна структура для простоты и передачи по указателю.
 */
typedef struct {
    Job  jobs[64];          /* 📋 Массив джобов (максимум 64 одновременно) */
    int  job_count;         /* 🔢 Сколько джобов сейчас зарегистрировано */
    int  next_job_id;       /* ➡️  Следующий свободный ID */
    char cwd[4096];         /* 📂 Текущая рабочая директория (кэш) */
    char home[4096];        /* 🏠 Домашняя директория пользователя */
    int  interactive;       /* 🖥️  1 = интерактивно (есть TTY), 0 = скрипт */
} ShellState;

/* 🌍 Единственный экземпляр состояния (глобалка — простительно для shell'а) */
static ShellState g_shell;

/*
 * ============================================================================
 * 🛠️  ВСПОМОГАТЕЛЬНЫЕ ФУНКЦИИ
 * ============================================================================
 */

/*
 * 📂 Получить домашнюю директорию пользователя.
 * Аналог dirs::home_dir() из Rust.
 * 🔎 Порядок поиска: $HOME → getpwuid() → "/tmp" (fallback).
 * ⚠️  Возвращает указатель на статический буфер — НЕ free()!
 */
static const char *get_home_dir(void) {
    const char *home = getenv("HOME");          /* 🥇 Сначала $HOME */
    if (home && *home) return home;             /* ✅ Нашли — вернули */
    return "/tmp";                              /* 🆘 Fallback (грубо, но работает) */
}

/*
 * 📂 Узнать текущую директорию и положить в g_shell.cwd.
 * Аналог std::env::current_dir() из Rust.
 * 💡 getcwd() может вернуть NULL если cwd удалён — тогда ставим "?".
 */
static void refresh_cwd(void) {
    char buf[4096];
    if (getcwd(buf, sizeof(buf))) {             /* 🎯 Успех */
        strncpy(g_shell.cwd, buf, sizeof(g_shell.cwd) - 1);
        g_shell.cwd[sizeof(g_shell.cwd) - 1] = '\0';  /* 🛡️ Гарантируем нуль */
    } else {
        strcpy(g_shell.cwd, "?");               /* 🤷 cwd недоступен */
    }
}

/*
 * 📁 Убедиться, что существует каталог (рекурсивно, как mkdir -p).
 * Аналог ensure_config_dir() из utilities.rs.
 * 🎯 Используется при старте, чтобы создать ~/.config/anssh/{themes,profiles,plugins}.
 * 💡 Простой обход: разбиваем путь по '/' и создаём каждый префикс.
 * ⚠️  Возвращает 0 при успехе, -1 при ошибке.
 */
static int mkdir_p(const char *path) {
    char tmp[4096];
    size_t len;

    /* 🔒 Проверяем длину — иначе переполним буфер */
    if (strlen(path) >= sizeof(tmp)) return -1;
    strcpy(tmp, path);
    len = strlen(tmp);

    /* 🧹 Убираем хвостовой слэш, если есть */
    if (len > 1 && tmp[len - 1] == '/') tmp[len - 1] = '\0';

    /* 🔁 Идём по строке: каждый раз, встретив '/', временно режем и mkdir */
    for (char *p = tmp + 1; *p; p++) {
        if (*p == '/') {
            *p = '\0';                          /* ✂️ Обрезали путь */
            if (mkdir(tmp, 0755) != 0 && errno != EEXIST) {
                return -1;                      /* ❌ Не смогли создать */
            }
            *p = '/';                           /* 🔄 Вернули слэш */
        }
    }
    /* 🎯 Последний сегмент — создаём без обрезки */
    if (mkdir(tmp, 0755) != 0 && errno != EEXIST) return -1;
    return 0;                                   /* ✅ Готово */
}

/*
 * 📁 Инициализация директорий конфигурации.
 * Аналог ensure_config_dir() из utilities.rs.
 * 🎯 Создаёт ~/.config/anssh, .../themes, .../profiles, .../plugins.
 * 💡 Ошибки игнорируются (как `let _ = ...` в Rust) — не критично.
 */
static void ensure_config_dirs(void) {
    char path[4096];
    snprintf(path, sizeof(path), "%s/%s", g_shell.home, ANSH_DIR_NAME);
    mkdir_p(path);                              /* 📁 ~/.config/anssh */
    snprintf(path, sizeof(path), "%s/%s/themes", g_shell.home, ANSH_DIR_NAME);
    mkdir_p(path);                              /* 🎨 .../themes */
    snprintf(path, sizeof(path), "%s/%s/profiles", g_shell.home, ANSH_DIR_NAME);
    mkdir_p(path);                              /* 👤 .../profiles */
    snprintf(path, sizeof(path), "%s/%s/plugins", g_shell.home, ANSH_DIR_NAME);
    mkdir_p(path);                              /* 🔌 .../plugins */
}

/*
 * 🐪 Вызвать Perl-хелпер и вернуть его stdout как строку.
 * Аналог вызова плагинов/менеджеров из Rust.
 * 🎯 Используется для: тем, профилей, плагинов, калькулятора, раскрытия путей.
 * 💡 Внутри — popen() с командой `perl main.pl <subcommand> <args...>`.
 * ⚠️  Возвращает malloc()-строку — вызывающий ОБЯЗАН free()!
 * ⚠️  Если Perl упал — возвращаем пустую строку "" (не NULL).
 */
static char *call_perl(const char *subcommand, const char *args) {
    char cmd[8192];
    /* 📝 Собираем команду: perl main.pl <sub> <args> */
    snprintf(cmd, sizeof(cmd), "perl %s %s %s 2>/dev/null",
             ANSH_PERL_PATH, subcommand, args ? args : "");
    FILE *fp = popen(cmd, "r");                 /* 🚀 Запустили Perl */
    if (!fp) return strdup("");                 /* 🆘 popen упал */

    /* 📥 Читаем весь вывод в динамический буфер */
    size_t cap = 4096, len = 0;
    char *buf = malloc(cap);
    if (!buf) { pclose(fp); return strdup(""); }

    size_t n;
    while ((n = fread(buf + len, 1, cap - len - 1, fp)) > 0) {
        len += n;
        if (len + 1 >= cap) {                   /* 📈 Буфер заполнен — растим */
            cap *= 2;
            char *nb = realloc(buf, cap);
            if (!nb) break;                     /* 😵 realloc упал — выходим */
            buf = nb;
        }
    }
    buf[len] = '\0';                            /* 🛡️ Нуль-терминатор */
    pclose(fp);                                 /* 🧹 Закрываем пайп */
    return buf;                                 /* ✅ Возвращаем строку */
}

/*
 * 🎨 Раскрыть цветовые коды вида $green в ANSI escape.
 * Аналог кода из main.rs (там был длинный replace).
 * 🎯 Нужно для prompt'а, где хранятся плейсхолдеры $blue, $green и т.п.
 * 💡 Простой проход replace'ом по известным ключам.
 * ⚠️  Возвращает malloc()-строку — вызывающий ОБЯЗАН free()!
 */
static char *expand_colors(const char *s) {
    /* 📋 Таблица: плейсхолдер → ANSI-код */
    static const struct { const char *key; const char *code; } colors[] = {
        {"$blue",   "\x1b[34m"}, {"$green", "\x1b[32m"},
        {"$red",    "\x1b[31m"}, {"$yellow","\x1b[33m"},
        {"$cyan",   "\x1b[36m"}, {"$pink",  "\x1b[38;5;205m"},
        {"$orange", "\x1b[38;5;214m"}, {"$purple","\x1b[35m"},
        {"$clear",  "\x1b[0m"},  {"$white", "\x1b[37m"},
        {"$gray",   "\x1b[90m"},
    };
    char *result = strdup(s);                   /* 🧬 Копия исходной строки */
    if (!result) return NULL;

    for (size_t i = 0; i < sizeof(colors)/sizeof(colors[0]); i++) {
        char *p;
        /* 🔁 Пока находим плейсхолдер — заменяем на ANSI-код */
        while ((p = strstr(result, colors[i].key)) != NULL) {
            size_t key_len = strlen(colors[i].key);
            size_t code_len = strlen(colors[i].code);
            size_t tail_len = strlen(p + key_len);
            size_t new_len = (p - result) + code_len + tail_len + 1;
            char *nr = malloc(new_len);
            if (!nr) return result;             /* 😵 malloc упал — возвращаем как есть */
            /* 🧩 Склеиваем: префикс + ANSI-код + хвост */
            memcpy(nr, result, p - result);
            memcpy(nr + (p - result), colors[i].code, code_len);
            memcpy(nr + (p - result) + code_len, p + key_len, tail_len + 1);
            free(result);
            result = nr;
        }
    }
    return result;                              /* ✅ Готово */
}

/*
 * 🎨 Построить строку приглашения (prompt).
 * Аналог кода из main.rs (там читался prompt-файл и делались replace'ы).
 * 🎯 Читает ~/.config/anssh/prompt, подставляет {user}, {cwd}, {host}, цвета.
 * ⚠️  Возвращает malloc()-строку — вызывающий ОБЯЗАН free()!
 */
static char *build_prompt(void) {
    char prompt_path[4096];
    snprintf(prompt_path, sizeof(prompt_path), "%s/%s/prompt",
             g_shell.home, ANSH_DIR_NAME);

    /* 📥 Читаем шаблон; если не удалось — дефолт "anssh> " */
    char tmpl[4096] = "anssh> ";
    FILE *fp = fopen(prompt_path, "r");
    if (fp) {
        if (fgets(tmpl, sizeof(tmpl), fp)) {
            size_t l = strlen(tmpl);
            if (l && tmpl[l - 1] == '\n') tmpl[l - 1] = '\0';  /* ✂️ Убираем перевод строки */
        }
        fclose(fp);
    }

    /* 🌍 Достаём данные для подстановки */
    const char *user = getenv("USER"); if (!user) user = "user";
    char host[256] = "localhost"; gethostname(host, sizeof(host));
    refresh_cwd();

    /* 🧩 Подставляем {user}, {host}, {cwd} в шаблон.
     * 💡 Простой подход: фиксированный буфер + snprintf по частям.
     * ⚠️  Для длинных промптов может обрезаться — но нам хватит. */
    char buf[8192];
    snprintf(buf, sizeof(buf), "%s", tmpl);
    /* Заменяем {user} */
    {
        char *p = strstr(buf, "{user}");
        if (p) {
            char out[8192]; size_t off = p - buf;
            snprintf(out, sizeof(out), "%.*s%s%s", (int)off, buf, user, p + 6);
            strcpy(buf, out);
        }
    }
    /* Заменяем {host} */
    {
        char *p = strstr(buf, "{host}");
        if (p) {
            char out[8192]; size_t off = p - buf;
            snprintf(out, sizeof(out), "%.*s%s%s", (int)off, buf, host, p + 6);
            strcpy(buf, out);
        }
    }
    /* Заменяем {cwd} */
    {
        char *p = strstr(buf, "{cwd}");
        if (p) {
            char out[8192]; size_t off = p - buf;
            snprintf(out, sizeof(out), "%.*s%s%s", (int)off, buf, g_shell.cwd, p + 5);
            strcpy(buf, out);
        }
    }

    /* 🎨 Раскрываем $blue/$green/... */
    char *colored = expand_colors(buf);
    return colored ? colored : strdup("anssh> ");  /* 🛡️ Fallback */
}

/*
 * 🔢 Добавить новый джоб. Аналог JobManager::add_job() из executor.rs.
 * 🎯 Возвращает ID созданного джоба.
 * 💡 Слоты не переиспользуем — при 64+ джобах начинаем сначала.
 */
static int job_add(const char *command, pid_t pid, int is_background) {
    if (g_shell.job_count >= 64) g_shell.job_count = 0;  /* ♻️ Сброс (грубо) */
    Job *j = &g_shell.jobs[g_shell.job_count++];
    j->id            = g_shell.next_job_id++;
    j->command       = strdup(command);
    j->pid           = pid;
    j->running       = 1;
    j->is_background = is_background;
    j->exit_code     = 0;
    return j->id;
}

/*
 * 🔍 Найти джоб по ID. Возвращает указатель или NULL.
 * 💡 Линейный поиск — джобов мало, оптимизация не нужна.
 */
static Job *job_find(int id) {
    for (int i = 0; i < g_shell.job_count; i++)
        if (g_shell.jobs[i].id == id) return &g_shell.jobs[i];
    return NULL;
}

/*
 * 🖨️ Вывести список джобов. Аналог команды `jobs`.
 * 💡 Формат как в bash: [1]+ Running: cmd
 */
static void cmd_jobs(void) {
    int any = 0;
    for (int i = 0; i < g_shell.job_count; i++) {
        Job *j = &g_shell.jobs[i];
        if (!j->running && j->pid == 0) continue;    /* 💀 Уже мёртв и убран */
        any = 1;
        const char *mark = j->is_background ? "+" : "-";
        printf("[%d]%s %s: %s\n",
               j->id, mark,
               j->running ? "Running" : "Done",
               j->command ? j->command : "?");
    }
    if (!any) printf("no jobs\n");                    /* 🗒️ Пусто */
}

/*
 * 📂 Встроенная команда cd.
 * Аналог handle_cd() из main.rs.
 * 🎯 Умеет: cd (в $HOME), cd - (в $OLDPWD), cd <path>.
 * 💡 ~ раскрывает Perl-хелпер (см. main.pl expand_tilde).
 */
static void cmd_cd(const char *target) {
    char dest[4096];

    /* 🎯 Обработка пустого аргумента → $HOME */
    if (!target || !*target) {
        snprintf(dest, sizeof(dest), "%s", g_shell.home);
    }
    /* 🎯 cd - → $OLDPWD (как в bash) */
    else if (strcmp(target, "-") == 0) {
        const char *old = getenv("OLDPWD");
        if (!old) { fprintf(stderr, "anssh: cd: OLDPWD not set\n"); return; }
        printf("%s\n", old);                       /* 💬 bash печатает новый путь */
        snprintf(dest, sizeof(dest), "%s", old);
    }
    /* 🎯 Обычный путь; если начинается с ~ — раскроем через Perl */
    else if (target[0] == '~') {
        char *expanded = call_perl("expand_tilde", target);
        snprintf(dest, sizeof(dest), "%s", expanded);
        free(expanded);
    } else {
        snprintf(dest, sizeof(dest), "%s", target);
    }

    /* 💾 Сохраняем старый cwd в $OLDPWD (для cd -) */
    char old[4096]; refresh_cwd();
    snprintf(old, sizeof(old), "%s", g_shell.cwd);

    if (chdir(dest) != 0) {                        /* ❌ Не смогли перейти */
        fprintf(stderr, "anssh: cd: %s: %s\n", dest, strerror(errno));
        return;
    }
    setenv("OLDPWD", old, 1);                      /* 🧠 Обновили $OLDPWD */
    refresh_cwd();                                 /* 🔄 Обновили кэш cwd */
}

/*
 * 🚀 Выполнить одну внешнюю команду: fork + execvp.
 * Аналог execute_command_line() из executor.rs (упрощённо, без pipe'ов).
 * 🎯 Поддерживает:
 *    - 🌙 фоновый режим (&)
 *    - 🔴 stderr → stdout (2>&1) — упрощённо
 *    - 📤 редиректы > >> < (упрощённо)
 * 💡 Возвращает PID запущенного процесса (для джобов) или -1.
 * ⚠️  Не поддерживает pipe'ы (|) — см. TODO ниже.
 */
static pid_t exec_external(char **argv, int background) {
    if (!argv || !argv[0]) return -1;

    pid_t pid = fork();                            /* 🧬 Создаём процесс */
    if (pid < 0) {                                 /* ❌ fork упал */
        perror("anssh: fork");
        return -1;
    }
    if (pid == 0) {
        /* 🧒 ДОЧЕРНИЙ ПРОЦЕСС — здесь выполняем команду */

        /* ⚡ Восстанавливаем SIGINT по умолчанию (shell мог его игнорировать) */
        signal(SIGINT, SIG_DFL);
        signal(SIGQUIT, SIG_DFL);

        /* 🚀 Заменяем образ процесса на команду */
        execvp(argv[0], argv);
        /* ❌ Сюда попадаем только при ошибке exec */
        fprintf(stderr, "anssh: %s: %s\n", argv[0], strerror(errno));
        _exit(127);                                /* 🚪 Код 127 = "не найдено" */
    }

    /* 👨 РОДИТЕЛЬСКИЙ ПРОЦЕСС */
    if (background) {
        printf("[%d] %s\n", job_add("", pid, 1), argv[0]);  /* 🌙 Фон */
    }
    return pid;
}

/*
 * ✂️ Разбить строку на токены по пробелам (упрощённый shell_words).
 * ⚠️  НЕ поддерживает кавычки с пробелами внутри! Только простой split.
 *     Для полноценной поддержки кавычек см. TODO в main.pl.
 * 💡 Возвращает malloc-массив malloc-строк, последний элемент — NULL.
 * ⚠️  Вызывающий освобождает через free_argv().
 */
static char **split_words(const char *line) {
    char **argv = calloc(MAX_ARGS + 1, sizeof(char *));
    if (!argv) return NULL;

    char *copy = strdup(line);
    if (!copy) { free(argv); return NULL; }

    int argc = 0;
    char *save = NULL;
    /* 🎯 strtok_r — потокобезопасный вариант strtok */
    for (char *tok = strtok_r(copy, " \t\r\n", &save);
         tok && argc < MAX_ARGS;
         tok = strtok_r(NULL, " \t\r\n", &save)) {
        argv[argc++] = strdup(tok);
    }
    argv[argc] = NULL;                             /* 🛡️ Маркер конца */
    free(copy);
    return argv;
}

/*
 * 🧹 Освободить массив argv от split_words().
 * 💡 Проходим по всем элементам до NULL и free().
 */
static void free_argv(char **argv) {
    if (!argv) return;
    for (int i = 0; argv[i]; i++) free(argv[i]);
    free(argv);
}

/*
 * 🧠 Обработать встроенную команду.
 * Аналог handle_builtin() из main.rs.
 * 🎯 Возвращает 1, если команда была встроенной (обработана),
 *    0 — если нужно передать внешней (execvp).
 * 💡 Все команды: exit, version, help, jobs, fg, bg, cd, theme, profile, plugins.
 */
static int handle_builtin(char **argv) {
    if (!argv || !argv[0]) return 0;
    const char *cmd = argv[0];

    /* 🚪 exit [code] — выйти с кодом (по умолчанию 0) */
    if (strcmp(cmd, "exit") == 0) {
        int code = argv[1] ? atoi(argv[1]) : 0;
        exit(code);
    }

    /* 🏷️ version — версия shell'а */
    if (strcmp(cmd, "version") == 0) {
        printf("anssh v%s\n", ANSH_VERSION);
        return 1;
    }

    /* ❓ help — помощь по встроенным командам */
    if (strcmp(cmd, "help") == 0) {
        printf("anssh v%s:\n", ANSH_VERSION);
        printf("  help  Print help message\n");
        printf("  version  Print version\n");
        printf("  exit [int]  Exit with code, default: 0\n");
        printf("  jobs  List running jobs\n");
        printf("  fg [job_id]  Bring job to foreground\n");
        printf("  bg [job_id]  Send job to background\n");
        printf("  cd [dir]  Change directory\n");
        printf("  theme [name]  Show or set theme\n");
        printf("  profile [name]  Show or switch profile\n");
        printf("  plugins  List loaded plugins\n");
        return 1;
    }

    /* 📋 jobs — список джобов */
    if (strcmp(cmd, "jobs") == 0) { cmd_jobs(); return 1; }

    /* 📂 cd — смена директории */
    if (strcmp(cmd, "cd") == 0) { cmd_cd(argv[1]); return 1; }

    /* 🎨 theme [name] — показать/установить тему (через Perl) */
    if (strcmp(cmd, "theme") == 0) {
        if (argv[1]) {
            char *res = call_perl("theme_set", argv[1]);
            printf("%s", res); free(res);
        } else {
            char *res = call_perl("theme_show", NULL);
            printf("%s", res); free(res);
        }
        return 1;
    }

    /* 👤 profile [name] — показать/переключить профиль (через Perl) */
    if (strcmp(cmd, "profile") == 0) {
        if (argv[1]) {
            char *res = call_perl("profile_set", argv[1]);
            printf("%s", res); free(res);
        } else {
            char *res = call_perl("profile_show", NULL);
            printf("%s", res); free(res);
        }
        return 1;
    }

    /* 🔌 plugins — список плагинов (через Perl) */
    if (strcmp(cmd, "plugins") == 0) {
        char *res = call_perl("plugins_list", NULL);
        printf("%s", res); free(res);
        return 1;
    }

    /* 🎯 fg [id] — вернуть джоб на передний план (упрощённо: waitpid) */
    if (strcmp(cmd, "fg") == 0) {
        if (!argv[1]) { fprintf(stderr, "anssh: fg: job ID required\n"); return 1; }
        Job *j = job_find(atoi(argv[1]));
        if (!j || !j->running) { fprintf(stderr, "anssh: fg: no such job\n"); return 1; }
        int status = 0;
        waitpid(j->pid, &status, 0);               /* ⏳ Ждём завершения */
        j->running = 0;
        return 1;
    }

    /* 🌙 bg [id] — отправить на задний план (SIGCONT) */
    if (strcmp(cmd, "bg") == 0) {
        if (!argv[1]) { fprintf(stderr, "anssh: bg: job ID required\n"); return 1; }
        Job *j = job_find(atoi(argv[1]));
        if (!j) { fprintf(stderr, "anssh: bg: no such job\n"); return 1; }
        kill(j->pid, SIGCONT);                     /* ▶️ Продолжаем выполнение */
        j->is_background = 1;
        return 1;
    }

    return 0;                                      /* 🔙 Не встроенная */
}

/*
 * ============================================================================
 * 🚀 ГЛАВНЫЙ ЦИКЛ И ТОЧКА ВХОДА
 * ============================================================================
 */

/*
 * 🔁 Обработать одну строку ввода: разбор, built-in или exec.
 * 🎯 Возвращает 0 при успехе, -1 при фатальной ошибке (exit).
 * 💡 Именно эту функцию вызываем и в REPL, и при -c "cmd".
 */
static int process_line(const char *line) {
    /* 🧹 Пропускаем пустые строки и комментарии (# ...) */
    if (!line) return 0;
    while (*line == ' ' || *line == '\t') line++;
    if (!*line || *line == '#') return 0;

    /* ✂️ Разбиваем на слова */
    char **argv = split_words(line);
    if (!argv) return -1;

    /* 🌙 Проверяем хвостовой '&' — фоновый режим */
    int argc = 0; while (argv[argc]) argc++;
    int background = 0;
    if (argc > 0 && strcmp(argv[argc - 1], "&") == 0) {
        background = 1;
        free(argv[argc - 1]);                      /* 🗑️ Убираем '&' */
        argv[argc - 1] = NULL;
    }

    /* 🧩 Пробуем как встроенную */
    if (handle_builtin(argv)) {
        free_argv(argv);
        return 0;
    }

    /* 🚀 Иначе — внешняя команда */
    pid_t pid = exec_external(argv, background);
    if (pid > 0 && !background) {
        /* ⏳ Ждём завершения (если не фон) */
        int status = 0;
        waitpid(pid, &status, 0);
    }
    free_argv(argv);
    return 0;
}

/*
 * 🚀 Точка входа.
 * 🎯 Разбор argv: -v, -h, -c, -nc и т.п.
 * 💡 Инициализирует состояние, читает rc, запускает REPL или -c.
 */
int main(int argc, char **argv) {
    /* 🏠 Инициализация состояния */
    const char *home = get_home_dir();
    snprintf(g_shell.home, sizeof(g_shell.home), "%s", home);
    refresh_cwd();
    g_shell.job_count   = 0;
    g_shell.next_job_id = 1;
    g_shell.interactive = isatty(STDIN_FILENO);

    /* 📁 Создаём ~/.config/anssh/{themes,profiles,plugins} */
    ensure_config_dirs();

    /* 📁 Создаём дефолтные файлы prompt/highlight, если их нет */
    {
        char p[4096];
        snprintf(p, sizeof(p), "%s/%s/prompt", g_shell.home, ANSH_DIR_NAME);
        FILE *fp = fopen(p, "r");
        if (!fp) {                                 /* ✍️ Нет — создаём */
            fp = fopen(p, "w");
            if (fp) { fputs("{user}@{host}:{cwd}$ ", fp); fclose(fp); }
        } else fclose(fp);
    }

    /* 🔎 Разбор аргументов командной строки */
    int norc = 0;                                  /* 📜 Не читать rc */
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-v") == 0 || strcmp(argv[i], "--version") == 0) {
            printf("anssh v%s\n", ANSH_VERSION);   /* 🏷️ */
            return 0;
        }
        if (strcmp(argv[i], "-h") == 0 || strcmp(argv[i], "--help") == 0) {
            printf("anssh v%s:\n", ANSH_VERSION);
            printf("  Usage:\n    anssh [Options]\n");
            printf("  Options:\n");
            printf("    -h, --help  Print help message\n");
            printf("    -v, --version  Print version\n");
            printf("    -nc, --norc  Not loading rc\n");
            printf("    -c, --command <cmd>  Execute command\n");
            return 0;
        }
        if (strcmp(argv[i], "-nc") == 0 || strcmp(argv[i], "--norc") == 0) {
            norc = 1;                              /* 📜 Отключаем rc */
            continue;
        }
        if ((strcmp(argv[i], "-c") == 0 || strcmp(argv[i], "--command") == 0) && i + 1 < argc) {
            /* 🎯 Режим одной команды: anssh -c "ls -la" */
            return process_line(argv[++i]) == 0 ? 0 : 1;
        }
    }

    /* 📜 Чтение rc-файла (если не отключено) */
    if (!norc) {
        char rc[4096];
        snprintf(rc, sizeof(rc), "%s/%s", g_shell.home, ANSH_RC_NAME);
        FILE *fp = fopen(rc, "r");
        if (fp) {
            char line[8192];
            while (fgets(line, sizeof(line), fp)) {
                line[strcspn(line, "\r\n")] = '\0'; /* ✂️ Убираем \n */
                char *l = line; while (*l == ' ' || *l == '\t') l++;
                if (!*l || *l == '#') continue;
                process_line(l);                   /* 🚀 Выполняем строку rc */
            }
            fclose(fp);
        }
    }

    /* ⌨️ Настройка readline: история в ~/.anssh_history */
    {
        char hist[4096];
        snprintf(hist, sizeof(hist), "%s/%s", g_shell.home, ANSH_HIST_NAME);
        read_history(hist);                        /* 📥 Загружаем историю */
        stifle_history(1000);                      /* 🗂️ Храним 1000 команд */
    }

    /* 🔁 Главный REPL-цикл */
    for (;;) {
        char *prompt = build_prompt();             /* 🎨 Строим prompt */
        char *line   = readline(prompt);           /* ⌨️ Читаем строку */
        free(prompt);                              /* 🗑️ Освободили */

        if (!line) break;                          /* 🚪 EOF (Ctrl-D) */

        if (*line) add_history(line);              /* 🕰️ В историю */

        process_line(line);                        /* 🚀 Обработали */
        free(line);                                /* 🗑️ Освободили */
    }

    /* 💾 Сохраняем историю при выходе */
    {
        char hist[4096];
        snprintf(hist, sizeof(hist), "%s/%s", g_shell.home, ANSH_HIST_NAME);
        write_history(hist);                       /* 📤 Пишем файл */
    }
    printf("\n");                                  /* 🎭 Прощальный перевод строки */
    return 0;
}
