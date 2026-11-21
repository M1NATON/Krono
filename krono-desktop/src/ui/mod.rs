pub mod board_dnd;
pub mod board_view;
pub mod calendar_widget;
pub mod canvas_view;
pub mod collection_board;
pub mod collection_view;
pub mod command_palette;
pub mod components;
pub mod dialogs;
pub mod editor_view;
pub mod embedded_board;
pub mod graph_view;
pub mod icons;
pub mod image_cache;
pub mod inspector;
pub mod markdown_assist;
pub mod markdown_editor;
pub mod markdown_preview;
pub mod mobile_nav;
pub mod motion;
pub mod settings_view;
pub mod sidebar;
pub mod table_view;
pub mod tasks_view;
pub mod theme;
pub mod topbar;
pub mod vault_map_view;
pub mod viewport_2d;

use crate::config::SlashTrigger;
use krono_core::{collection::CollectionViewKind, daily::SimpleDate, note::NoteStatus};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutClass {
    Desktop, // >= 900.0
    Compact, // 600.0 .. 900.0
    Mobile,  // < 600.0
}

impl LayoutClass {
    pub fn from_width(w: f32) -> Self {
        if w >= 900.0 {
            Self::Desktop
        } else if w >= 600.0 {
            Self::Compact
        } else {
            Self::Mobile
        }
    }

    pub fn is_mobile(self) -> bool {
        self == Self::Mobile
    }

    pub fn is_desktop(self) -> bool {
        self == Self::Desktop
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Editor,
    Tasks,
    Table,
    Board,
    Graph,
    Overview,
    Collection,
    Canvas,
    Settings,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavTarget {
    Note(String),
    Collection(String),
    Canvas(String),
    View(ViewMode),
}

#[derive(Clone, Debug)]
pub struct RecentTabItem {
    pub target: NavTarget,
    pub title: String,
    pub kind: krono_core::path::VaultFileKind,
    pub dirty: bool,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CreateItemKind {
    #[default]
    Note,
    Board,
    Table,
    Canvas,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PagesKindFilter {
    #[default]
    All,
    Notes,
    Boards,
    Tables,
    Canvases,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PagesSortOrder {
    #[default]
    NameAsc,
    ModifiedDesc,
    NameDesc,
}

pub struct PagesTreeUiState<'a> {
    pub filter: &'a mut PagesKindFilter,
    pub sort: &'a mut PagesSortOrder,
    pub filter_open: &'a mut bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorMode {
    Edit,
    Split,
    Read,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableSort {
    ModifiedDesc,
    ModifiedAsc,
    TitleAsc,
    TitleDesc,
    StatusAsc,
    StatusDesc,
    TagsAsc,
    TagsDesc,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoardSort {
    #[default]
    Manual,
    ModifiedDesc,
    ModifiedAsc,
    TitleAsc,
    TitleDesc,
}

#[derive(Clone, Copy, Debug)]
pub struct TableColumnWidths {
    pub name: f32,
    pub status: f32,
    pub tags: f32,
    pub path: f32,
    pub modified: f32,
}

impl Default for TableColumnWidths {
    fn default() -> Self {
        Self {
            name: 260.0,
            status: 140.0,
            tags: 180.0,
            path: 180.0,
            modified: 110.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
    Appearance,
    Editor,
    MarkdownHelp,
    Storage,
    GitHub,
    Shortcuts,
}

#[derive(Clone, Copy, Debug)]
pub struct EditorPrefs {
    pub font_size: f32,
    pub heading_serif: bool,
    pub max_width: f32,
    pub show_footer: bool,
    pub md_slash_menu: bool,
    pub md_wikilink_autocomplete: bool,
    pub md_auto_lists: bool,
    pub md_auto_pairs: bool,
    pub md_show_toolbar: bool,
    pub md_slash_trigger: SlashTrigger,
}

pub struct CalendarUiState<'a> {
    pub open: bool,
    pub year: &'a mut i32,
    pub month: &'a mut u32,
    pub today: SimpleDate,
    pub daily_dates: &'a HashSet<SimpleDate>,
}

#[derive(Clone, Copy)]
pub struct SyncUiState<'a> {
    pub busy: bool,
    pub angle: f32,
    pub configured: bool,
    pub status_text: &'a str,
    pub pending_changes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VaultCounts {
    pub notes: usize,
    pub boards: usize,
    pub tables: usize,
    pub canvases: usize,
    pub tags: usize,
    pub links: usize,
}

impl VaultCounts {
    #[allow(dead_code)]
    pub fn total_items(&self) -> usize {
        self.notes + self.boards + self.tables + self.canvases
    }
}

pub struct EditorDocState<'a> {
    pub selected_path: Option<&'a str>,
    pub title: &'a mut String,
    pub status: &'a mut Option<NoteStatus>,
    pub tags: &'a mut Vec<String>,
    pub pinned: &'a mut bool,
    pub extra_lines: &'a mut Vec<(String, String)>,
    pub body: &'a mut String,
    pub modified: i64,
    pub external_conflict: bool,
}

#[derive(Clone, Copy)]
pub struct TopbarDocContext<'a> {
    pub selected_note_path: Option<&'a str>,
    pub note_title: &'a str,
    pub note_dirty: bool,
    pub active_collection_path: Option<&'a str>,
    pub collection_title: &'a str,
    pub collection_dirty: bool,
    pub collection_view: CollectionViewKind,
    pub active_canvas_path: Option<&'a str>,
    pub canvas_title: &'a str,
    pub canvas_dirty: bool,
}

#[derive(Clone, Debug)]
pub enum UiAction {
    OpenNote(String),
    CreateNote {
        title: String,
        status: Option<NoteStatus>,
        tags: Vec<String>,
    },
    OpenOrCreateWikilink(String),
    SaveCurrentNote,
    CommitCurrentNoteTitle,
    UpdateNoteStatus {
        path: String,
        status: Option<NoteStatus>,
    },
    MoveNoteInBoard {
        path: String,
        status: NoteStatus,
        insert_before: Option<String>,
    },
    ShiftNoteInBoard {
        path: String,
        delta: isize,
    },
    NavBack,
    NavForward,
    OpenNavTarget(NavTarget),
    CloseRecentTab(NavTarget),
    OpenCreateModalInFolder(String),
    CreateInFolder {
        folder: String,
        kind: CreateItemKind,
    },
    StartCreateSubfolder(String),
    SendTaskToBoard {
        task_idx: usize,
        board_path: String,
        move_task: bool,
    },
    RefreshAssetAudit,
    DeleteUnusedAssets,
    DeleteSingleAsset(String),
    StartRename(String),
    DuplicateNote(String),
    RequestDelete(String),
    TogglePin(String),
    OpenOrCreateDailyNote(SimpleDate),
    OpenTodayNote,
    ToggleCalendar,
    SetViewMode(ViewMode),
    ReturnToNotesTab,
    SetEditorMode(EditorMode),
    MobileShowNotesList,
    OpenCreateModal(Option<CreateItemKind>),
    CreateFromModal {
        kind: CreateItemKind,
        title: String,
        folder: String,
        preset_id: String,
    },
    ToggleSidebar,
    ToggleInspector,
    ToggleTheme,
    ToggleLang,
    OpenPalette,
    OpenSettings(SettingsTab),
    CloseSettings,
    SyncNow,
    TestGitHubConnection,
    SaveGitHubToken,
    DeleteGitHubToken,
    SetTagFilter(Option<String>),
    ToggleFolder(String),
    MarkDirty,
    ToggleTask(usize),
    ToggleVaultTask {
        path: String,
        task_idx: usize,
    },
    CarryOverDailyTasks,
    ConvertTaskToNote {
        source_path: String,
        task_idx: usize,
    },
    ConvertRecordToNote {
        record_id: String,
    },
    CopyText(String),
    ApplyVaultDir(String),
    OpenVaultFolder,
    RebuildIndex,
    ConfigChanged,
    ExternalReloadFromDisk,
    ExternalKeepMine,
    ExternalSaveConflictCopy,
    OpenCollection(String),
    CreateCollection {
        title: String,
        kind: CollectionViewKind,
    },
    SetCollectionView(CollectionViewKind),
    SaveCurrentCollection,
    MarkCollectionDirty,
    CollectionExternalReload,
    CollectionExternalKeepMine,
    CollectionExternalSaveConflictCopy,
    OpenCanvas(String),
    SaveCurrentCanvas,
    MarkCanvasDirty,
    CanvasExternalReload,
    CanvasExternalKeepMine,
    CanvasExternalSaveConflictCopy,
    ExportCanvasToMarkdown,
    OpenGraphForNode(String),
    PickImageForEditor,
    PickImageForCanvas,
    PasteClipboardImage,
    OpenImageLightbox(String),
    CreateLinkedBoardForCurrentNote,
    EmbeddedBoard(embedded_board::EmbeddedBoardAction),
    CheckForUpdates,
    InstallUpdate,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_responsive_breakpoints() {
        assert_eq!(LayoutClass::from_width(360.0), LayoutClass::Mobile);
        assert_eq!(LayoutClass::from_width(599.0), LayoutClass::Mobile);
        assert_eq!(LayoutClass::from_width(600.0), LayoutClass::Compact);
        assert_eq!(LayoutClass::from_width(899.0), LayoutClass::Compact);
        assert_eq!(LayoutClass::from_width(900.0), LayoutClass::Desktop);
        assert_eq!(LayoutClass::from_width(1280.0), LayoutClass::Desktop);
    }
}
