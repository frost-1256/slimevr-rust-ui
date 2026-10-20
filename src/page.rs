//! The navigation destinations, in sidebar order.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Dashboard,
    Trackers,
    Reset,
    Proportions,
    Checklist,
    Settings,
    Console,
    Server,
}

impl Page {
    pub const ALL: [(Page, &'static str); 8] = [
        (Page::Dashboard, "ダッシュボード"),
        (Page::Trackers, "トラッカー"),
        (Page::Reset, "リセット"),
        (Page::Proportions, "プロポーション"),
        (Page::Checklist, "チェックリスト"),
        (Page::Settings, "設定"),
        (Page::Console, "シリアル / FW"),
        (Page::Server, "サーバー"),
    ];

    pub fn label(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(page, _)| *page == self)
            .map(|(_, label)| *label)
            .unwrap_or("?")
    }

    /// Material Symbols name, matching `Page::ALL` order.
    pub fn icon(self) -> &'static str {
        match self {
            Page::Dashboard => "dashboard",
            Page::Trackers => "sensors",
            Page::Reset => "restart_alt",
            Page::Proportions => "straighten",
            Page::Checklist => "checklist",
            Page::Settings => "settings",
            Page::Console => "terminal",
            Page::Server => "dns",
        }
    }
}
