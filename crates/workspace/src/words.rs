//! Theme words for the launcher (STORY-083, issue #134): the themed
//! ("fortress", "net") and plain ("project", "host") vocabularies the
//! launcher switches between.
//!
//! The design's Decision 1 puts a `pub(crate) struct Words` of `&'static str`
//! fields and small `fn`s here, with consts `THEMED` and `PLAIN`; the user
//! strings that used to be hardcoded in `launcher.rs` (`THIS_MAC`, "All",
//! "folder", `open_label`, `HostState::word`) live here as table-driven data
//! instead. Only the words the v3 canvas actually shows differently between
//! the two tables move here (Decision 4) — the scanning/failed/empty/
//! no-match texts, the key hints, "<k> matches", "CELLWORKS", the NAV/CMD mode
//! badges and the PATH/BRANCH/FILES captions stay single strings in `ui.rs`.

use crate::launcher::HostState;

/// One vocabulary the launcher can speak: the themed Netrunner words, or
/// STORY-082's plain ones. `&'static str` fields for the words that are
/// simply picked; a few `fn` pointers for the handful whose shape — not just
/// which literal is picked — differs between the two tables (Decision 1
/// rejects a lookup by key, which the compiler cannot check).
// Order of work step 1 (this file) builds the full table the design's
// Requirements 7 and 8 name; step 2 is what rewires `ui.rs` to read every
// field and call every method below. Until then, several are only read by
// `Launcher`'s own fields that this step wires (`this_mac_label`,
// `this_mac_location`, `all_chip`, `branchless`, `open_label`) — the rest sit
// unread, which `dead_code` would otherwise flag on a crate built with
// `-D warnings`.
#[allow(dead_code)]
pub(crate) struct Words {
    /// The header's eyebrow line, above the counts (requirement 7); empty in
    /// plain words, which never had one (STORY-082 ASSUMED, recorded in
    /// `design.md`'s Open questions).
    pub(crate) header_eyebrow: &'static str,
    /// The preview pane's eyebrow line, above the project's name; empty in
    /// plain words for the same reason as `header_eyebrow`.
    pub(crate) preview_eyebrow: &'static str,
    /// The "All" chip's label.
    pub(crate) all_chip: &'static str,
    /// The local location's name at the label position: the chip and the
    /// group heading (STORY-082 requirements 3 and 5).
    pub(crate) this_mac_label: &'static str,
    /// The local location's name at the body position: the preview pane's
    /// "`<kind>` · `<location>`" line (STORY-082 requirement 7).
    pub(crate) this_mac_location: &'static str,
    /// The recent row's heading.
    pub(crate) recent_heading: &'static str,
    /// What a local project with no known branch is called (STORY-082
    /// requirement 5).
    pub(crate) branchless: &'static str,
    /// What the inspector calls a local repository (requirement 9).
    pub(crate) local_repo_kind: &'static str,
    /// What the inspector calls a remote project (requirement 9).
    pub(crate) remote_kind: &'static str,
    /// The disabled "Add project ⌘N" button's label (requirement 1).
    pub(crate) add_project_button: &'static str,
    /// The search field's placeholder (STORY-082 requirement 1).
    pub(crate) search_placeholder: &'static str,
    /// The CTA's label when the project can be opened.
    pub(crate) open_action: &'static str,
    /// The CTA's subtitle, and the open action's label, when it cannot be
    /// opened because the host is not connected.
    pub(crate) connect_first: &'static str,
    /// Put before the project's name as the enabled CTA's subtitle ("Jack
    /// into cellworks"); empty in plain words, where the CTA has no subtitle.
    pub(crate) open_subtitle: &'static str,
    /// The disabled "Sync · coming soon" button's label.
    pub(crate) sync_button: &'static str,
    /// The ICE/offline card's heading, shown when the selected project's host
    /// is offline.
    pub(crate) ice_heading: &'static str,
    /// Prefixed to a notice below the preview card (requirement 4); empty in
    /// plain words.
    pub(crate) notice_prefix: &'static str,
    /// The status bar's mode text.
    pub(crate) status_text: &'static str,
    /// "<n> fortresses"/"<n> projects" — the count word for one project.
    project_singular: &'static str,
    /// The same, for more than one.
    project_plural: &'static str,
    /// "<m> nets"/"<m> hosts" — the count word for one host.
    host_singular: &'static str,
    /// The same, for more than one.
    host_plural: &'static str,
    host_word_connected: &'static str,
    host_word_checking: &'static str,
    host_word_offline: &'static str,
    /// The ICE/offline card's body, given the host's name: differs in shape,
    /// not just in which literal is picked, so this is a function rather than
    /// a template string.
    offline_body_fn: fn(&str) -> String,
    /// A skipped-host notice, given `notice_prefix`, the host and the reason:
    /// the plain wording is not merely the themed one without its prefix
    /// (word order differs too), so this is a function.
    notice_fn: fn(&str, &str, &str) -> String,
    /// A remote host's group heading, given its name and state: themed adds
    /// the state after the upper-cased name, plain is the bare name
    /// (`Launcher::groups()` itself is unchanged; this is what a later step's
    /// view uses to draw the heading).
    host_group_heading_fn: fn(&Words, &str, HostState) -> String,
}

impl Words {
    /// The header's "<n> fortresses · <m> nets" / "<n> projects · <m> hosts"
    /// (requirement 7 and 8), singular at exactly one for either count
    /// independently (requirement 9).
    ///
    /// Only this file's own unit tests call it until step 2 wires `ui.rs`'s
    /// header to it, hence `#[allow(dead_code)]`.
    #[allow(dead_code)]
    pub(crate) fn counts(&self, projects: usize, hosts: usize) -> String {
        let project_word = if projects == 1 {
            self.project_singular
        } else {
            self.project_plural
        };
        let host_word = if hosts == 1 {
            self.host_singular
        } else {
            self.host_plural
        };
        format!("{projects} {project_word} · {hosts} {host_word}")
    }

    /// The open action's label: `connected` is the same condition
    /// `Launcher::open_label` already computes — no host (a local project),
    /// or a remote project whose host is connected.
    pub(crate) fn open_label(&self, connected: bool) -> &'static str {
        if connected {
            self.open_action
        } else {
            self.connect_first
        }
    }

    /// A host state's word, as a chip or a group heading reads it — this is
    /// `HostState::word`'s old match, relocated here by Decision 1.
    pub(crate) fn host_state_word(&self, state: HostState) -> &'static str {
        match state {
            HostState::Connected => self.host_word_connected,
            HostState::Checking => self.host_word_checking,
            HostState::Offline => self.host_word_offline,
        }
    }

    /// A remote host's group heading, given its name and state.
    ///
    /// Not yet called outside this file's own doc comment — step 2 wires
    /// `ui.rs`'s group heading to it — hence `#[allow(dead_code)]`.
    #[allow(dead_code)]
    pub(crate) fn host_group_heading(&self, host: &str, state: HostState) -> String {
        (self.host_group_heading_fn)(self, host, state)
    }

    /// The ICE/offline card's body, given the offline host's name.
    #[allow(dead_code)]
    pub(crate) fn offline_body(&self, host: &str) -> String {
        (self.offline_body_fn)(host)
    }

    /// A skipped-host notice, given the host's name and why it was skipped.
    #[allow(dead_code)]
    pub(crate) fn notice(&self, host: &str, reason: &str) -> String {
        (self.notice_fn)(self.notice_prefix, host, reason)
    }
}

fn themed_offline_body(host: &str) -> String {
    format!("The {host} net does not answer. Its fortresses stay listed but cannot be breached.")
}

fn plain_offline_body(host: &str) -> String {
    format!("The {host} host is not reachable right now.")
}

fn themed_notice(prefix: &str, host: &str, reason: &str) -> String {
    format!("{prefix}Skipped the {host} net: {reason}")
}

fn plain_notice(_prefix: &str, host: &str, reason: &str) -> String {
    format!("Skipped the host {host}: {reason}")
}

fn themed_host_group_heading(words: &Words, host: &str, state: HostState) -> String {
    format!(
        "{} NET · {}",
        host.to_uppercase(),
        words.host_state_word(state)
    )
}

fn plain_host_group_heading(_words: &Words, host: &str, _state: HostState) -> String {
    host.to_owned()
}

/// The Netrunner theme words, on by default (requirement 6).
pub(crate) const THEMED: Words = Words {
    header_eyebrow: "// FORTRESS SELECT",
    preview_eyebrow: "FORTRESS",
    all_chip: "ALL",
    this_mac_label: "LOCAL NET",
    this_mac_location: "local net",
    recent_heading: "RECENT",
    branchless: "block",
    local_repo_kind: "git fortress",
    remote_kind: "block",
    add_project_button: "ADD FORTRESS ⌘N",
    search_placeholder: "cd fortress, block or host:/path",
    open_action: "OPEN FORTRESS",
    connect_first: "Connect the net first",
    open_subtitle: "Jack into",
    sync_button: "SYNC · COMING SOON",
    ice_heading: "ICE // NET OFFLINE",
    notice_prefix: "ICE // ",
    status_text: "fortress select",
    project_singular: "fortress",
    project_plural: "fortresses",
    host_singular: "net",
    host_plural: "nets",
    host_word_connected: "CONNECTED",
    host_word_checking: "CHECKING",
    host_word_offline: "OFFLINE",
    offline_body_fn: themed_offline_body,
    notice_fn: themed_notice,
    host_group_heading_fn: themed_host_group_heading,
};

/// STORY-082's plain words, unchanged except where requirement 8 names a
/// difference (the open action, its offline subtitle, the ICE card heading,
/// the notice prefix and the status text).
pub(crate) const PLAIN: Words = Words {
    header_eyebrow: "",
    preview_eyebrow: "",
    all_chip: "All",
    this_mac_label: "This Mac",
    this_mac_location: "This Mac",
    recent_heading: "Recent",
    branchless: "folder",
    local_repo_kind: "git repository",
    remote_kind: "remote",
    add_project_button: "Add project ⌘N",
    search_placeholder: "projects, folders or host:/path, ⌘K",
    open_action: "Open project",
    connect_first: "Connect the host first",
    open_subtitle: "",
    sync_button: "Sync · coming soon",
    ice_heading: "Host offline",
    notice_prefix: "",
    status_text: "launcher",
    project_singular: "project",
    project_plural: "projects",
    host_singular: "host",
    host_plural: "hosts",
    host_word_connected: "connected",
    host_word_checking: "checking",
    host_word_offline: "offline",
    offline_body_fn: plain_offline_body,
    notice_fn: plain_notice,
    host_group_heading_fn: plain_host_group_heading,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Requirement 7: the themed header reads "<n> fortresses · <m> nets",
    /// plural for more than one of either count.
    #[test]
    fn themed_counts_are_plural_above_one() {
        assert_eq!(THEMED.counts(4, 2), "4 fortresses · 2 nets");
    }

    /// Requirement 9: "1 fortress · 1 net" — the singular exactly at one,
    /// for both counts together.
    #[test]
    fn themed_counts_are_singular_at_one() {
        assert_eq!(THEMED.counts(1, 1), "1 fortress · 1 net");
    }

    /// Requirement 8: the plain header reads "<n> projects · <m> hosts",
    /// STORY-082's unchanged text, plural for more than one of either count.
    #[test]
    fn plain_counts_are_plural_above_one() {
        assert_eq!(PLAIN.counts(4, 2), "4 projects · 2 hosts");
    }

    /// Requirement 9: "1 project · 1 host" — the singular exactly at one.
    #[test]
    fn plain_counts_are_singular_at_one() {
        assert_eq!(PLAIN.counts(1, 1), "1 project · 1 host");
    }

    /// Decision 1's relocation: `HostState::word`'s old match moves here
    /// unchanged for plain words, with a themed, upper-cased counterpart
    /// (STORY-082 ASSUMED (Q-1): the word alone, never a glyph).
    #[test]
    fn host_state_word_is_relocated_from_host_state_word() {
        assert_eq!(THEMED.host_state_word(HostState::Connected), "CONNECTED");
        assert_eq!(THEMED.host_state_word(HostState::Checking), "CHECKING");
        assert_eq!(THEMED.host_state_word(HostState::Offline), "OFFLINE");

        assert_eq!(PLAIN.host_state_word(HostState::Connected), "connected");
        assert_eq!(PLAIN.host_state_word(HostState::Checking), "checking");
        assert_eq!(PLAIN.host_state_word(HostState::Offline), "offline");
    }
}
