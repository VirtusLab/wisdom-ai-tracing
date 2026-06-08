//! Minimal, conservative classification of Bash command strings for policy
//! decisions. The first consumer is the verification-phase gate, which must
//! recognize a standalone `tracevault verify-start` (the command that opens
//! the phase) WITHOUT being fooled by compounds that hide other commands.

/// True iff `command` is a single, simple invocation of `tracevault <subcommand>`
/// — no shell compounding of any kind. Rejecting compounds is the security
/// crux: `tracevault verify-start && rm -rf /` must NOT match, or the gate
/// could be tricked into ignoring a hidden command.
pub fn is_standalone_tracevault_subcommand(command: &str, subcommand: &str) -> bool {
    let trimmed = command.trim();

    // Reject anything that could chain, redirect, background, or substitute a
    // second command. ("&&"/"||" are covered by "&"/"|".) VT/FF are added
    // because Rust's split_whitespace treats them as whitespace but Bash does not.
    const COMPOUND_MARKERS: &[&str] = &[
        ";", "|", "&", "\n", "\r", "\u{0B}", "\u{0C}", "`", "$(", ">", "<",
    ];
    if COMPOUND_MARKERS.iter().any(|m| trimmed.contains(m)) {
        return false;
    }

    let mut tokens = trimmed.split_whitespace();
    let Some(arg0) = tokens.next() else {
        return false;
    };
    // An env-assignment prefix (NAME=value) makes the *next* token the real
    // command, so `EVIL=/tracevault evil` would run `evil`. Reject it.
    if arg0.contains('=') {
        return false;
    }
    // basename of argv[0] must be exactly `tracevault` (allow an absolute/relative path prefix).
    let prog = arg0.rsplit(['/', '\\']).next().unwrap_or(arg0);
    if prog != "tracevault" {
        return false;
    }
    // argv[1] must be exactly the expected subcommand.
    tokens.next() == Some(subcommand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_bare_invocation() {
        assert!(is_standalone_tracevault_subcommand(
            "tracevault verify-start",
            "verify-start"
        ));
    }

    #[test]
    fn matches_with_path_and_flags() {
        assert!(is_standalone_tracevault_subcommand(
            "/usr/local/bin/tracevault verify-start --session-id abc",
            "verify-start"
        ));
    }

    #[test]
    fn matches_with_surrounding_whitespace() {
        assert!(is_standalone_tracevault_subcommand(
            "  tracevault   verify-start  ",
            "verify-start"
        ));
    }

    #[test]
    fn rejects_compound_with_and() {
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault verify-start && rm -rf /",
            "verify-start"
        ));
    }

    #[test]
    fn rejects_compound_with_semicolon_and_pipe() {
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault verify-start; evil",
            "verify-start"
        ));
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault verify-start | tee log",
            "verify-start"
        ));
    }

    #[test]
    fn rejects_command_substitution_and_background() {
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault verify-start $(evil)",
            "verify-start"
        ));
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault verify-start &",
            "verify-start"
        ));
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault verify-start `evil`",
            "verify-start"
        ));
    }

    #[test]
    fn rejects_disguised_argv0() {
        assert!(!is_standalone_tracevault_subcommand(
            "echo tracevault verify-start",
            "verify-start"
        ));
    }

    #[test]
    fn rejects_wrong_subcommand() {
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault check",
            "verify-start"
        ));
    }

    #[test]
    fn rejects_newline_compound() {
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault verify-start\nrm -rf /",
            "verify-start"
        ));
    }

    #[test]
    fn rejects_env_assignment_prefix_bypass() {
        // `NAME=value cmd` runs `cmd`, not tracevault — must not be accepted.
        assert!(!is_standalone_tracevault_subcommand(
            "EVIL=/tracevault verify-start",
            "verify-start"
        ));
        assert!(!is_standalone_tracevault_subcommand(
            "X=/tracevault evil-cmd",
            "verify-start"
        ));
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault=x verify-start",
            "verify-start"
        ));
    }

    #[test]
    fn rejects_vertical_tab_and_form_feed_separators() {
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault verify-start\u{0B}evil",
            "verify-start"
        ));
        assert!(!is_standalone_tracevault_subcommand(
            "tracevault verify-start\u{0C}evil",
            "verify-start"
        ));
    }
}
