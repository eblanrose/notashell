#!/usr/bin/env perl
# ============================================================================
# 🐪 main.pl — PERL-ХЕЛПЕР ДЛЯ ANSsh
# ============================================================================
# 📝 Этот скрипт вызывается из main.c через popen("perl main.pl <sub> ...")
#    Он реализует "сложную" логику, которую на C писать больно:
#      - 🎨 Темы (чтение/запись .theme файлов)
#      - 👤 Профили (.profile файлы)
#      - 🔌 Плагины (заглушки)
#      - 🧮 Калькулятор
#      - 🏠 Раскрытие ~ (tilde)
#      - 📂 Раскрытие {a,b} (braces) — опционально
#
# 🎯 КОНТРАКТ: main.pl <subcommand> [args...]
#    Возвращает текст в STDOUT (его читает popen).
#    Ошибки — в STDERR (main.c их игнорирует через 2>/dev/null).
# ============================================================================

use strict;                                      # 🛡️ Строгие переменные
use warnings;                                    # ⚠️  Предупреждения
use utf8;                                        # 🌍 UTF-8 в исходнике
binmode STDOUT, ":encoding(UTF-8)";              # 🌍 UTF-8 в выводе

# 📁 Пути конфигурации (те же, что в main.c)
my $HOME = $ENV{HOME} // "/tmp";                 # 🏠 Домашняя директория
my $CFG  = "$HOME/.config/anssh";                # 📁 Каталог конфигурации
my $THEMES_DIR   = "$CFG/themes";                # 🎨 Темы
my $PROFILES_DIR = "$CFG/profiles";              # 👤 Профили
my $PLUGINS_DIR  = "$CFG/plugins";               # 🔌 Плагины

# ============================================================================
# 🧰 ВСПОМОГАТЕЛЬНЫЕ ФУНКЦИИ
# ============================================================================

# 📂 Прочитать файл целиком в строку. Возвращает undef при ошибке.
sub slurp {
    my ($path) = @_;
    open my $fh, '<', $path or return undef;
    local $/;                                    # 🐌 "Slurp mode" — читаем всё
    my $content = <$fh>;
    close $fh;
    return $content // '';
}

# 💾 Записать строку в файл. Возвращает 1 при успехе, 0 при ошибке.
sub spew {
    my ($path, $content) = @_;
    open my $fh, '>', $path or return 0;
    print $fh $content;
    close $fh;
    return 1;
}

# 📁 Убедиться, что каталог существует (аналог mkdir -p).
sub ensure_dir {
    my ($dir) = @_;
    return if -d $dir;                           # ✅ Уже есть
    # 🪄 File::Path::make_path — рекурсивное создание
    require File::Path;
    File::Path::make_path($dir);
}

# ============================================================================
# 🎨 ТЕМЫ (аналог themes.rs + ThemeManager)
# ============================================================================

# 🗂️ Хранилище текущей темы (в памяти процесса Perl).
# ⚠️  Между вызовами НЕ сохраняется — каждый вызов main.pl читает файл заново.
my $CURRENT_THEME_FILE = "$CFG/current_theme";

# 📥 Прочитать текущую тему (имя) из файла.
sub theme_current_name {
    my $name = slurp($CURRENT_THEME_FILE);
    return ($name && $name =~ /\S/) ? $name : "default";
}

# 🎨 Встроенные темы (аналог load_builtin_themes в Rust).
# 📋 Каждая тема — hashref с ключами prompt/syntax/color.
sub theme_builtins {
    return {
        default => {
            name => "default",
            prompt => {
                template   => "{user}@{host}:{cwd}\$ ",
                user_color => "green",
                host_color => "cyan",
                cwd_color  => "blue",
                git_color  => "yellow",
                normal_color => "white",
            },
            syntax => {
                command_color  => "cyan",
                arg_color      => "white",
                string_color   => "green",
                variable_color => "yellow",
                operator_color => "magenta",
                comment_color  => "gray",
                error_color    => "red",
                path_color     => "orange",
            },
            color => {
                fg => "white", bg => "black", cursor => "white",
                selection => "blue", status_ok => "green",
                status_warn => "yellow", status_err => "red",
            },
        },
        hacker => {
            name => "hacker",
            prompt => {
                template   => "[\$green{user}\$clear@\$cyan{host}\$clear]\$gray:\$reset{cwd}\$clear> ",
                user_color => "green", host_color => "cyan",
                cwd_color  => "green",  git_color  => "yellow",
                normal_color => "gray",
            },
            syntax => {
                command_color => "green", arg_color => "white",
                string_color  => "yellow", variable_color => "cyan",
                operator_color => "magenta", comment_color => "gray",
                error_color    => "red",     path_color    => "orange",
            },
            color => {
                fg => "green", bg => "black", cursor => "green",
                selection => "darkgreen", status_ok => "green",
                status_warn => "yellow",  status_err => "red",
            },
        },
        ocean => {
            name => "ocean",
            prompt => {
                template   => "\$blue{user}\$clear@\$cyan{host}\$clear:\$blue{cwd}\$clear \$cyan➜\$clear ",
                user_color => "blue", host_color => "cyan",
                cwd_color  => "blue",  git_color  => "yellow",
                normal_color => "white",
            },
            syntax => {
                command_color => "cyan", arg_color => "white",
                string_color  => "green", variable_color => "yellow",
                operator_color => "magenta", comment_color => "gray",
                error_color    => "red",     path_color    => "cyan",
            },
            color => {
                fg => "white", bg => "blue", cursor => "white",
                selection => "darkblue", status_ok => "green",
                status_warn => "yellow", status_err => "red",
            },
        },
        sunset => {
            name => "sunset",
            prompt => {
                template   => "{user}@💻 {cwd} ",
                user_color => "orange", host_color => "magenta",
                cwd_color  => "yellow", git_color  => "green",
                normal_color => "white",
            },
            syntax => {
                command_color => "orange", arg_color => "white",
                string_color  => "cyan",   variable_color => "yellow",
                operator_color => "magenta", comment_color => "gray",
                error_color    => "red",     path_color    => "cyan",
            },
            color => {
                fg => "white", bg => "black", cursor => "yellow",
                selection => "darkyellow", status_ok => "green",
                status_warn => "orange",   status_err => "red",
            },
        },
    };
}

# 🎨 Загрузить тему по имени. Возвращает hashref или undef.
# 🔎 Порядок: встроенные → файл $THEMES_DIR/<name>.theme.
sub theme_load {
    my ($name) = @_;
    my $builtins = theme_builtins();
    return $builtins->{$name} if exists $builtins->{$name};

    # 📂 Пробуем прочитать .theme файл
    my $path = "$THEMES_DIR/$name.theme";
    my $content = slurp($path);
    return undef unless defined $content;

    # 📋 Парсим key=value построчно (аналог Theme::parse)
    my $theme = { name => $name, prompt => {}, syntax => {}, color => {} };
    for my $line (split /\n/, $content) {
        $line =~ s/^\s+|\s+$//g;                 # ✂️ trim
        next if $line eq '' || $line =~ /^#/;    # 🧹 пропускаем пустые/комменты
        my ($k, $v) = split /=/, $line, 2;
        next unless defined $v;
        $k =~ s/\s+//g; $v =~ s/^\s+|\s+$//g;
        if    ($k eq 'name')            { $theme->{name} = $v }
        elsif ($k =~ /^prompt\.(.+)$/)  { $theme->{prompt}{$1} = $v }
        elsif ($k =~ /^syntax\.(.+)$/)  { $theme->{syntax}{$1} = $v }
        elsif ($k =~ /^color\.(.+)$/)   { $theme->{color}{$1}  = $v }
    }
    return $theme;
}

# 🎨 Сохранить тему в файл (аналог Theme::to_file).
sub theme_save {
    my ($name, $theme) = @_;
    ensure_dir($THEMES_DIR);
    my $out = "# ansSH Theme: $name\n\n";
    $out .= "name=$name\n\n# Prompt Settings\n";
    for my $k (qw(template user_color host_color cwd_color git_color normal_color)) {
        $out .= "prompt.$k=" . ($theme->{prompt}{$k} // '') . "\n";
    }
    $out .= "\n# Syntax Highlighting\n";
    for my $k (qw(command arg string variable operator comment error path)) {
        my $v = $theme->{syntax}{$k . '_color'} // $theme->{syntax}{$k} // '';
        $out .= "syntax.$k=$v\n";
    }
    $out .= "\n# Color Scheme\n";
    for my $k (qw(fg bg cursor selection status_ok status_warn status_err)) {
        $out .= "color.$k=" . ($theme->{color}{$k} // '') . "\n";
    }
    spew("$THEMES_DIR/$name.theme", $out);
}

# 🎨 SUBCOMMAND: theme_show — показать текущую тему.
sub cmd_theme_show {
    print "Current theme: " . theme_current_name() . "\n";
}

# 🎨 SUBCOMMAND: theme_set <name> — установить тему.
sub cmd_theme_set {
    my ($name) = @_;
    my $theme = theme_load($name);
    unless ($theme) {
        print STDERR "anssh: theme '$name' not found\n";
        my $b = theme_builtins();
        print "Available themes: " . join(", ", sort keys %$b) . "\n";
        return;
    }
    ensure_dir($CFG);
    spew($CURRENT_THEME_FILE, $name);
    print "Theme set to: $name\n";
    # 🎯 Опционально: обновить prompt-файл шаблоном темы
    if ($theme->{prompt}{template}) {
        my $p = "$CFG/prompt";
        spew($p, $theme->{prompt}{template});
    }
}

# ============================================================================
# 👤 ПРОФИЛИ (аналог themes.rs + ProfileManager)
# ============================================================================

# 📥 Прочитать активный профиль (имя).
sub profile_current_name {
    my $p = "$CFG/current_profile";
    my $name = slurp($p);
    return ($name && $name =~ /\S/) ? $name : "default";
}

# 👤 Загрузить профиль по имени. Возвращает hashref или undef.
sub profile_load {
    my ($name) = @_;
    my $path = "$PROFILES_DIR/$name.profile";
    my $content = slurp($path);
    return undef unless defined $content;
    my $profile = { name => $name, aliases => {}, env_vars => {},
                    theme => undef, prompt_template => undef };
    for my $line (split /\n/, $content) {
        $line =~ s/^\s+|\s+$//g;
        next if $line eq '' || $line =~ /^#/;
        if ($line =~ /^alias\s+(\S+?)=(.*)$/) {
            $profile->{aliases}{$1} = $2;
        } elsif ($line =~ /^export\s+(\S+?)=(.*)$/) {
            my ($k, $v) = ($1, $2);
            $v =~ s/^["']|["']$//g;              # ✂️ Убираем кавычки
            $profile->{env_vars}{$k} = $v;
        } elsif ($line =~ /^theme=(.*)$/) {
            $profile->{theme} = $1;
        } elsif ($line =~ /^prompt=(.*)$/) {
            $profile->{prompt_template} = $1;
        } elsif ($line =~ /^name=(.*)$/) {
            $profile->{name} = $1;
        }
    }
    return $profile;
}

# 👤 Сохранить профиль в файл (аналог Profile::to_file).
sub profile_save {
    my ($name, $profile) = @_;
    ensure_dir($PROFILES_DIR);
    my $out = "[profile]\nname=$name\n\n";
    if ($profile->{theme})           { $out .= "theme=$profile->{theme}\n\n" }
    if ($profile->{prompt_template}) { $out .= "prompt=$profile->{prompt_template}\n\n" }
    $out .= "# Environment Variables\n";
    for my $k (sort keys %{$profile->{env_vars}}) {
        $out .= "export $k=$profile->{env_vars}{$k}\n";
    }
    $out .= "\n# Aliases\n";
    for my $k (sort keys %{$profile->{aliases}}) {
        $out .= "alias $k=$profile->{aliases}{$k}\n";
    }
    spew("$PROFILES_DIR/$name.profile", $out);
}

# 👤 SUBCOMMAND: profile_show — показать текущий профиль.
sub cmd_profile_show {
    print "Current profile: " . profile_current_name() . "\n";
}

# 👤 SUBCOMMAND: profile_set <name> — переключить профиль.
sub cmd_profile_set {
    my ($name) = @_;
    my $p = profile_load($name);
    unless ($p) {
        print STDERR "anssh: profile '$name' not found\n";
        my @all;
        for my $f (glob("$PROFILES_DIR/*.profile")) {
            (my $n = $f) =~ s{.*/|\.profile$}{}g;
            push @all, $n;
        }
        print "Available profiles: " . join(", ", sort @all) . "\n";
        return;
    }
    ensure_dir($CFG);
    spew("$CFG/current_profile", $name);
    print "Profile set to: $name\n";

    # 🎯 Применить тему профиля, если указана
    if ($p->{theme}) { cmd_theme_set($p->{theme}) }

    # 🎯 Применить prompt, если указан
    if ($p->{prompt_template}) {
        spew("$CFG/prompt", $p->{prompt_template});
    }
}

# ============================================================================
# 🔌 ПЛАГИНЫ (аналог plugins.rs — упрощённо)
# ============================================================================

# 🔌 SUBCOMMAND: plugins_list — список плагинов.
# 💡 В Rust были встроенные (git, weather, datetime, calc, aliases).
#    Здесь — статический список + динамические из $PLUGINS_DIR.
sub cmd_plugins_list {
    print "Loaded plugins:\n";
    # 🎯 Встроенные "плагины" (заглушки для совместимости)
    my @builtin = (
        ["git",      "1.0.0", "Git integration - shows branch and status in prompt"],
        ["weather",  "1.0.0", "Shows current weather in prompt"],
        ["datetime", "1.0.0", "Shows current date/time"],
        ["calc",     "1.0.0", "Simple calculator"],
        ["aliases",  "1.0.0", "Built-in aliases"],
    );
    for my $p (@builtin) {
        printf "  %s (v%s) - %s\n", $p->[0], $p->[1], $p->[2];
    }
    # 📂 Динамические .so из $PLUGINS_DIR — просто перечисляем
    for my $f (glob("$PLUGINS_DIR/*.so"), glob("$PLUGINS_DIR/*.dylib")) {
        (my $n = $f) =~ s{.*/|\.\w+$}{}g;
        print "  $n (v1.0.0) - External plugin from $f\n";
    }
}

# ============================================================================
# 🧮 КАЛЬКУЛЯТОР (аналог CalcPlugin::evaluate_expression)
# ============================================================================

# 🧮 SUBCOMMAND: calc <expr> — вычислить выражение.
# 💡 Простая реализация: используем eval в безопасном окружении.
#    ⚠️  В продакшене — НЕ используйте eval на пользовательском вводе!
#    Здесь — учебный пример, поэтому допустимо с оговорками.
sub cmd_calc {
    my ($expr) = @_;
    $expr //= '';
    # 🧹 Оставляем только цифры, + - * / . ( ) и пробелы
    $expr =~ s/[^0-9+\-*\/.()\s]//g;
    if ($expr !~ /\d/) {
        print STDERR "Usage: calc <expression>\n";
        return;
    }
    # 🧮 Считаем через eval (рискованно, но для демо)
    my $result = eval $expr;
    if ($@) {
        print STDERR "Error: $@\n";
    } else {
        print "$result\n";
    }
}

# ============================================================================
# 🏠 РАСКРЫТИЕ ~ (tilde) — аналог expand_tilde из expansions.rs
# ============================================================================

# 🏠 SUBCOMMAND: expand_tilde <path> — раскрыть ~ в /home/user.
# 💡 Поддерживает: ~, ~/path, ~user/path, ~+/path (cwd), ~-/path (OLDPWD).
sub cmd_expand_tilde {
    my ($path) = @_;
    $path //= '';
    my $result = $path;

    if ($path =~ m{^~\+}) {                       # ~+ → cwd
        my $cwd = `pwd`; chomp $cwd;
        $result =~ s{^~\+}{$cwd};
    } elsif ($path =~ m{^~-}) {                   # ~- → OLDPWD
        my $old = $ENV{OLDPWD} // $HOME;
        $result =~ s{^~-}{$old};
    } elsif ($path =~ m{^~([A-Za-z0-9_-]+)}) {    # ~user → home of user
        my $user = $1;
        my $user_home = (getpwnam($user))[7] // $HOME;
        $result =~ s{^~$user}{$user_home};
    } elsif ($path =~ m{^~}) {                    # ~ → $HOME
        $result =~ s{^~}{$HOME};
    }
    print $result;
}

# ============================================================================
# 🎯 ГЛАВНЫЙ ДИСПЕТЧЕР SUBCOMMAND'ОВ
# ============================================================================

# 📋 Берём subcommand из @ARGV
my $sub = shift @ARGV // '';

# 🔀 Таблица: subcommand → кодовый блок
my %dispatch = (
    'theme_show'      => \&cmd_theme_show,
    'theme_set'       => sub { cmd_theme_set(shift @ARGV) },
    'profile_show'    => \&cmd_profile_show,
    'profile_set'     => sub { cmd_profile_set(shift @ARGV) },
    'plugins_list'    => \&cmd_plugins_list,
    'calc'            => sub { cmd_calc(join ' ', @ARGV) },
    'expand_tilde'    => sub { cmd_expand_tilde(shift @ARGV) },
);

# 🚀 Выполняем, если знаем; иначе — ошибка в STDERR.
if (exists $dispatch{$sub}) {
    $dispatch{$sub}->();
} else {
    print STDERR "anssh-perl: unknown subcommand '$sub'\n";
    print STDERR "Available: " . join(", ", sort keys %dispatch) . "\n";
    exit 1;
}

exit 0;                                          # ✅ Успех
