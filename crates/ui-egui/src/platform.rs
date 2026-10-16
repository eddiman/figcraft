//! Platform policy, separate from layout and document contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentMode {
    DesktopPaths,
    OwnedCopies,
    ExternalDocuments,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontDiscovery {
    System,
    EmbeddedOnly,
    Imported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformCapabilities {
    pub multiple_windows: bool,
    pub reveal_file: bool,
    pub local_session_hosting: bool,
    pub plugin_downloads: bool,
    pub document_mode: DocumentMode,
    pub font_discovery: FontDiscovery,
}

impl PlatformCapabilities {
    pub const fn desktop() -> Self {
        Self {
            multiple_windows: true,
            reveal_file: true,
            local_session_hosting: true,
            plugin_downloads: true,
            document_mode: DocumentMode::DesktopPaths,
            font_discovery: FontDiscovery::System,
        }
    }

    pub const fn web() -> Self {
        Self { document_mode: DocumentMode::Unavailable, ..Self::ipad() }
    }

    pub const fn ipad() -> Self {
        Self {
            multiple_windows: false,
            reveal_file: false,
            local_session_hosting: false,
            plugin_downloads: false,
            document_mode: DocumentMode::OwnedCopies,
            font_discovery: FontDiscovery::EmbeddedOnly,
        }
    }
}
