pub fn normalize(command: &str) -> String {
    command
        .chars()
        .filter(|c| *c != '\\')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn requires_root(command: &str) -> bool {
    let c = command.to_ascii_lowercase();
    c.split([';', '&', '|', '(', '`']).any(|segment| {
        segment
            .trim_start_matches([' ', '\'', '"', '$'])
            .starts_with("su ")
    }) || c.contains("/data/system")
        || (c.contains("/system/") && has_mutation(&c))
        || c.contains("/dev/block")
        || c.contains(" remount")
}

pub fn has_mutation(command: &str) -> bool {
    let c = command.to_ascii_lowercase();
    // Make command names inside substitutions visible to the conservative token
    // check without treating every benign `$(...)` query as a side effect.
    let lexical = c.replace("$(", " ").replace(['(', ')', '`'], " ");
    let tokens = shell_words::split(&lexical)
        .unwrap_or_else(|_| lexical.split_whitespace().map(str::to_owned).collect());
    let mutating = [
        "rm", "mv", "cp", "chmod", "chown", "mkdir", "rmdir", "touch", "ln", "truncate", "tee",
        "setprop", "kill", "pkill", "reboot", "shutdown", "halt", "poweroff", "mkfs", "dd",
    ];
    tokens.iter().any(|t| mutating.contains(&t.as_str()))
        || c.contains("sed -i")
        || c.contains("-delete")
        || c.contains("settings put")
        || c.contains("pm install")
        || c.contains("pm uninstall")
        || has_mutating_redirection(&c)
        || c.contains("mount -o")
}

fn has_mutating_redirection(command: &str) -> bool {
    command.match_indices('>').any(|(index, _)| {
        let mut target = command[index + 1..].trim_start();
        if let Some(rest) = target.strip_prefix('>') {
            target = rest.trim_start();
        }
        !is_discard_target(target) && !is_fd_duplication(target)
    })
}

fn is_discard_target(target: &str) -> bool {
    let (target, quote) = match target.chars().next() {
        Some(quote @ ('\'' | '"')) => (&target[quote.len_utf8()..], Some(quote)),
        _ => (target, None),
    };
    let Some(rest) = target.strip_prefix("/dev/null") else {
        return false;
    };
    let rest = match quote {
        Some(quote) => match rest.strip_prefix(quote) {
            Some(rest) => rest,
            None => return false,
        },
        None => rest,
    };
    is_shell_boundary(rest)
}

fn is_fd_duplication(target: &str) -> bool {
    let Some(rest) = target.strip_prefix('&') else {
        return false;
    };
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    digits > 0 && is_shell_boundary(&rest[digits..])
}

fn is_shell_boundary(rest: &str) -> bool {
    match rest.chars().next() {
        None => true,
        Some(c) => c.is_whitespace() || matches!(c, ';' | '|' | '&' | ')' | '`'),
    }
}
