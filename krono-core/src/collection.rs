use crate::daily::SimpleDate;
use crate::markdown::{extract_wikilinks, rewrite_wikilinks};
use crate::note::normalize_tags;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateFilterPreset {
    #[default]
    All,
    Overdue,
    Today,
    DueTodayOrOverdue,
    Next7Days,
    NoDate,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedCollectionView {
    pub id: String,
    pub name: String,
    pub view_kind: CollectionViewKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub query: String,
    #[serde(default)]
    pub date_preset: DateFilterPreset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select_col_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select_opt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_number_col_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_number: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionViewKind {
    #[default]
    Board,
    Table,
    Gallery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorToken {
    #[default]
    Slate,
    Blue,
    Green,
    Amber,
    Coral,
    Violet,
    Teal,
    Rose,
}

impl ColorToken {
    pub const ALL: [ColorToken; 8] = [
        Self::Slate,
        Self::Blue,
        Self::Green,
        Self::Amber,
        Self::Coral,
        Self::Violet,
        Self::Teal,
        Self::Rose,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectOption {
    pub id: String,
    pub label: String,
    pub color: ColorToken,
}

impl Default for SelectOption {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            color: ColorToken::Slate,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    #[default]
    Text,
    Number,
    Select,
    Checkbox,
    Date,
    NoteLink,
}

impl ColumnType {
    pub const ALL: [ColumnType; 6] = [
        Self::Text,
        Self::Number,
        Self::Select,
        Self::Checkbox,
        Self::Date,
        Self::NoteLink,
    ];
}

fn default_col_width() -> f32 {
    150.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnDef {
    pub id: String,
    pub name: String,
    pub col_type: ColumnType,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<SelectOption>,
    #[serde(default = "default_col_width")]
    pub width: f32,
}

impl Default for ColumnDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            col_type: ColumnType::Text,
            options: Vec::new(),
            width: default_col_width(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChecklistItem {
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

impl FieldValue {
    pub fn as_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Number(n) => {
                if n.is_finite() && n.fract().abs() < 1e-9 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            Self::Bool(b) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0.0,
            Self::Text(s) => matches!(
                s.trim().to_lowercase().as_str(),
                "true" | "1" | "yes" | "да" | "x"
            ),
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) if n.is_finite() => Some(*n),
            Self::Number(_) => None,
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Self::Text(s) => s.trim().replace(',', ".").parse::<f64>().ok(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Text(s) => s.trim().is_empty(),
            Self::Number(_) | Self::Bool(_) => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordItem {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checklist: Vec<ChecklistItem>,
    #[serde(default)]
    pub values: BTreeMap<String, FieldValue>,
    #[serde(default)]
    pub order: i64,
    #[serde(default)]
    pub created_at: i64,
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CollectionDoc {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub title: String,
    #[serde(default)]
    pub pinned: bool,
    pub default_view: CollectionViewKind,
    #[serde(default)]
    pub group_by_column: Option<String>,
    #[serde(default)]
    pub columns: Vec<ColumnDef>,
    #[serde(default)]
    pub records: Vec<RecordItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub saved_views: Vec<SavedCollectionView>,
}

impl Default for CollectionDoc {
    fn default() -> Self {
        Self {
            schema_version: 1,
            title: "Untitled".to_string(),
            pinned: false,
            default_view: CollectionViewKind::Board,
            group_by_column: None,
            columns: Vec::new(),
            records: Vec::new(),
            saved_views: Vec::new(),
        }
    }
}

impl CollectionDoc {
    pub fn new_board(title: &str, is_ru: bool) -> Self {
        let stage_col = ColumnDef {
            id: "stage".to_string(),
            name: if is_ru { "Этап" } else { "Stage" }.to_string(),
            col_type: ColumnType::Select,
            width: 148.0,
            options: if is_ru {
                vec![
                    SelectOption {
                        id: "opt_idea".into(),
                        label: "Идея".into(),
                        color: ColorToken::Slate,
                    },
                    SelectOption {
                        id: "opt_work".into(),
                        label: "В работе".into(),
                        color: ColorToken::Blue,
                    },
                    SelectOption {
                        id: "opt_review".into(),
                        label: "Проверка".into(),
                        color: ColorToken::Amber,
                    },
                    SelectOption {
                        id: "opt_done".into(),
                        label: "Готово".into(),
                        color: ColorToken::Green,
                    },
                ]
            } else {
                vec![
                    SelectOption {
                        id: "opt_idea".into(),
                        label: "Idea".into(),
                        color: ColorToken::Slate,
                    },
                    SelectOption {
                        id: "opt_work".into(),
                        label: "In Progress".into(),
                        color: ColorToken::Blue,
                    },
                    SelectOption {
                        id: "opt_review".into(),
                        label: "Review".into(),
                        color: ColorToken::Amber,
                    },
                    SelectOption {
                        id: "opt_done".into(),
                        label: "Done".into(),
                        color: ColorToken::Green,
                    },
                ]
            },
        };

        let priority_col = ColumnDef {
            id: "priority".to_string(),
            name: if is_ru { "Приоритет" } else { "Priority" }.to_string(),
            col_type: ColumnType::Select,
            width: 120.0,
            options: if is_ru {
                vec![
                    SelectOption {
                        id: "prio_high".into(),
                        label: "Высокий".into(),
                        color: ColorToken::Coral,
                    },
                    SelectOption {
                        id: "prio_med".into(),
                        label: "Средний".into(),
                        color: ColorToken::Amber,
                    },
                    SelectOption {
                        id: "prio_low".into(),
                        label: "Низкий".into(),
                        color: ColorToken::Teal,
                    },
                ]
            } else {
                vec![
                    SelectOption {
                        id: "prio_high".into(),
                        label: "High".into(),
                        color: ColorToken::Coral,
                    },
                    SelectOption {
                        id: "prio_med".into(),
                        label: "Medium".into(),
                        color: ColorToken::Amber,
                    },
                    SelectOption {
                        id: "prio_low".into(),
                        label: "Low".into(),
                        color: ColorToken::Teal,
                    },
                ]
            },
        };

        let due_col = ColumnDef {
            id: "due".to_string(),
            name: if is_ru { "Срок" } else { "Due Date" }.to_string(),
            col_type: ColumnType::Date,
            options: Vec::new(),
            width: 118.0,
        };

        let link_col = ColumnDef {
            id: "note_link".to_string(),
            name: if is_ru { "Заметка" } else { "Note" }.to_string(),
            col_type: ColumnType::NoteLink,
            options: Vec::new(),
            width: 164.0,
        };

        Self {
            schema_version: 1,
            title: title.trim().to_string(),
            pinned: false,
            default_view: CollectionViewKind::Board,
            group_by_column: Some("stage".to_string()),
            columns: vec![stage_col, priority_col, due_col, link_col],
            records: Vec::new(),
            saved_views: Vec::new(),
        }
    }

    pub fn new_table(title: &str, is_ru: bool) -> Self {
        let status_col = ColumnDef {
            id: "status".to_string(),
            name: if is_ru { "Статус" } else { "Status" }.to_string(),
            col_type: ColumnType::Select,
            width: 136.0,
            options: if is_ru {
                vec![
                    SelectOption {
                        id: "st_plan".into(),
                        label: "К выполнению".into(),
                        color: ColorToken::Slate,
                    },
                    SelectOption {
                        id: "st_active".into(),
                        label: "Активно".into(),
                        color: ColorToken::Blue,
                    },
                    SelectOption {
                        id: "st_done".into(),
                        label: "Готово".into(),
                        color: ColorToken::Green,
                    },
                ]
            } else {
                vec![
                    SelectOption {
                        id: "st_plan".into(),
                        label: "Planned".into(),
                        color: ColorToken::Slate,
                    },
                    SelectOption {
                        id: "st_active".into(),
                        label: "Active".into(),
                        color: ColorToken::Blue,
                    },
                    SelectOption {
                        id: "st_done".into(),
                        label: "Done".into(),
                        color: ColorToken::Green,
                    },
                ]
            },
        };

        let notes_col = ColumnDef {
            id: "details".to_string(),
            name: if is_ru { "Детали" } else { "Details" }.to_string(),
            col_type: ColumnType::Text,
            options: Vec::new(),
            width: 170.0,
        };

        let num_col = ColumnDef {
            id: "amount".to_string(),
            name: if is_ru { "Оценка" } else { "Score" }.to_string(),
            col_type: ColumnType::Number,
            options: Vec::new(),
            width: 96.0,
        };

        let check_col = ColumnDef {
            id: "done".to_string(),
            name: if is_ru { "Готово" } else { "Done" }.to_string(),
            col_type: ColumnType::Checkbox,
            options: Vec::new(),
            width: 78.0,
        };

        let date_col = ColumnDef {
            id: "date".to_string(),
            name: if is_ru { "Дата" } else { "Date" }.to_string(),
            col_type: ColumnType::Date,
            options: Vec::new(),
            width: 112.0,
        };

        let link_col = ColumnDef {
            id: "note".to_string(),
            name: if is_ru { "Заметка" } else { "Note" }.to_string(),
            col_type: ColumnType::NoteLink,
            options: Vec::new(),
            width: 156.0,
        };

        Self {
            schema_version: 1,
            title: title.trim().to_string(),
            pinned: false,
            default_view: CollectionViewKind::Table,
            group_by_column: Some("status".to_string()),
            columns: vec![status_col, notes_col, num_col, check_col, date_col, link_col],
            records: Vec::new(),
            saved_views: Vec::new(),
        }
    }

    /// Creates a custom Board from a curated preset (`"kanban"`, `"content"`, `"sprint"`, `"blank"`).
    pub fn new_board_preset(title: &str, preset_id: &str, is_ru: bool) -> Self {
        let mut doc = Self::new_board(title, is_ru);
        match preset_id {
            "content" => {
                if let Some(st) = doc.columns.iter_mut().find(|c| c.id == "stage") {
                    st.options = if is_ru {
                        vec![
                            SelectOption { id: "opt_idea".into(), label: "Идеи".into(), color: ColorToken::Violet },
                            SelectOption { id: "opt_script".into(), label: "Сценарий".into(), color: ColorToken::Blue },
                            SelectOption { id: "opt_shoot".into(), label: "Съёмка".into(), color: ColorToken::Amber },
                            SelectOption { id: "opt_edit".into(), label: "Монтаж".into(), color: ColorToken::Coral },
                            SelectOption { id: "opt_pub".into(), label: "Вышло".into(), color: ColorToken::Green },
                        ]
                    } else {
                        vec![
                            SelectOption { id: "opt_idea".into(), label: "Ideas".into(), color: ColorToken::Violet },
                            SelectOption { id: "opt_script".into(), label: "Script".into(), color: ColorToken::Blue },
                            SelectOption { id: "opt_shoot".into(), label: "Filming".into(), color: ColorToken::Amber },
                            SelectOption { id: "opt_edit".into(), label: "Editing".into(), color: ColorToken::Coral },
                            SelectOption { id: "opt_pub".into(), label: "Published".into(), color: ColorToken::Green },
                        ]
                    };
                }
                doc.add_column(
                    if is_ru { "Платформа" } else { "Platform" },
                    ColumnType::Text,
                    is_ru,
                );
                let r1 = doc.add_record(
                    if is_ru { "Выпуск #1: Главный обзор" } else { "Episode #1: Main Deep Dive" },
                    Some(("stage", "opt_idea")),
                    1,
                );
                let _ = doc.add_record(
                    if is_ru { "Шортс / Тизер" } else { "Short Teaser" },
                    Some(("stage", "opt_script")),
                    2,
                );
                if let Some(rec) = doc.records.iter_mut().find(|r| r.id == r1) {
                    rec.checklist.push(ChecklistItem {
                        text: if is_ru { "Собрать тезисы".into() } else { "Outline key hooks".into() },
                        done: false,
                    });
                }
            }
            "sprint" => {
                if let Some(st) = doc.columns.iter_mut().find(|c| c.id == "stage") {
                    st.name = if is_ru { "Статус" } else { "Sprint" }.to_string();
                    st.options = if is_ru {
                        vec![
                            SelectOption { id: "sp_back".into(), label: "Бэклог".into(), color: ColorToken::Slate },
                            SelectOption { id: "sp_work".into(), label: "В работе".into(), color: ColorToken::Blue },
                            SelectOption { id: "sp_rev".into(), label: "Ревью".into(), color: ColorToken::Amber },
                            SelectOption { id: "sp_rel".into(), label: "Релиз".into(), color: ColorToken::Green },
                        ]
                    } else {
                        vec![
                            SelectOption { id: "sp_back".into(), label: "Backlog".into(), color: ColorToken::Slate },
                            SelectOption { id: "sp_work".into(), label: "In Progress".into(), color: ColorToken::Blue },
                            SelectOption { id: "sp_rev".into(), label: "Review".into(), color: ColorToken::Amber },
                            SelectOption { id: "sp_rel".into(), label: "Shipped".into(), color: ColorToken::Green },
                        ]
                    };
                }
                doc.add_column(
                    if is_ru { "Оценка (ч)" } else { "Points" },
                    ColumnType::Number,
                    is_ru,
                );
                let _ = doc.add_record(
                    if is_ru { "Ключевая задача спринта" } else { "Core sprint deliverable" },
                    Some(("stage", "sp_work")),
                    1,
                );
            }
            "blank" => {
                if let Some(st) = doc.columns.iter_mut().find(|c| c.id == "stage") {
                    st.options = if is_ru {
                        vec![
                            SelectOption { id: "col_1".into(), label: "Сделать".into(), color: ColorToken::Slate },
                            SelectOption { id: "col_2".into(), label: "В процессе".into(), color: ColorToken::Blue },
                            SelectOption { id: "col_3".into(), label: "Готово".into(), color: ColorToken::Green },
                        ]
                    } else {
                        vec![
                            SelectOption { id: "col_1".into(), label: "To Do".into(), color: ColorToken::Slate },
                            SelectOption { id: "col_2".into(), label: "Doing".into(), color: ColorToken::Blue },
                            SelectOption { id: "col_3".into(), label: "Done".into(), color: ColorToken::Green },
                        ]
                    };
                }
            }
            _ => {}
        }
        doc
    }

    /// Creates a custom Table from a curated preset (`"general"`, `"subscriptions"`, `"reading"`, `"crm"`).
    pub fn new_table_preset(title: &str, preset_id: &str, is_ru: bool) -> Self {
        match preset_id {
            "subscriptions" => {
                let mut doc = Self {
                    schema_version: 1,
                    title: title.trim().to_string(),
                    pinned: false,
                    default_view: CollectionViewKind::Table,
                    group_by_column: Some("category".to_string()),
                    columns: vec![
                        ColumnDef {
                            id: "category".into(),
                            name: if is_ru { "Категория" } else { "Category" }.into(),
                            col_type: ColumnType::Select,
                            width: 140.0,
                            options: vec![
                                SelectOption { id: "cat_work".into(), label: if is_ru { "Работа" } else { "Work" }.into(), color: ColorToken::Blue },
                                SelectOption { id: "cat_cloud".into(), label: if is_ru { "Сервисы" } else { "Cloud" }.into(), color: ColorToken::Teal },
                                SelectOption { id: "cat_life".into(), label: if is_ru { "Личное" } else { "Personal" }.into(), color: ColorToken::Amber },
                            ],
                        },
                        ColumnDef {
                            id: "price".into(),
                            name: if is_ru { "Сумма / мес" } else { "Monthly" }.into(),
                            col_type: ColumnType::Number,
                            options: Vec::new(),
                            width: 112.0,
                        },
                        ColumnDef {
                            id: "active".into(),
                            name: if is_ru { "Активна" } else { "Active" }.into(),
                            col_type: ColumnType::Checkbox,
                            options: Vec::new(),
                            width: 82.0,
                        },
                        ColumnDef {
                            id: "renewal".into(),
                            name: if is_ru { "Продление" } else { "Renewal" }.into(),
                            col_type: ColumnType::Date,
                            options: Vec::new(),
                            width: 116.0,
                        },
                    ],
                    records: Vec::new(),
                    saved_views: Vec::new(),
                };
                let r1 = doc.add_record("Cloud Backup", Some(("category", "cat_cloud")), 1);
                if let Some(rec) = doc.records.iter_mut().find(|r| r.id == r1) {
                    rec.values.insert("price".into(), FieldValue::Number(10.0));
                    rec.values.insert("active".into(), FieldValue::Bool(true));
                }
                doc
            }
            "reading" => {
                let mut doc = Self {
                    schema_version: 1,
                    title: title.trim().to_string(),
                    pinned: false,
                    default_view: CollectionViewKind::Table,
                    group_by_column: Some("status".to_string()),
                    columns: vec![
                        ColumnDef {
                            id: "status".into(),
                            name: if is_ru { "Статус" } else { "Status" }.into(),
                            col_type: ColumnType::Select,
                            width: 136.0,
                            options: vec![
                                SelectOption { id: "rd_queue".into(), label: if is_ru { "В планах" } else { "Queued" }.into(), color: ColorToken::Slate },
                                SelectOption { id: "rd_now".into(), label: if is_ru { "Изучаю" } else { "Reading" }.into(), color: ColorToken::Blue },
                                SelectOption { id: "rd_done".into(), label: if is_ru { "Прочитано" } else { "Finished" }.into(), color: ColorToken::Green },
                            ],
                        },
                        ColumnDef {
                            id: "author".into(),
                            name: if is_ru { "Автор" } else { "Author" }.into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 160.0,
                        },
                        ColumnDef {
                            id: "rating".into(),
                            name: if is_ru { "Оценка" } else { "Rating" }.into(),
                            col_type: ColumnType::Number,
                            options: Vec::new(),
                            width: 86.0,
                        },
                        ColumnDef {
                            id: "note".into(),
                            name: if is_ru { "Конспект" } else { "Notes" }.into(),
                            col_type: ColumnType::NoteLink,
                            options: Vec::new(),
                            width: 156.0,
                        },
                    ],
                    records: Vec::new(),
                    saved_views: Vec::new(),
                };
                let _ = doc.add_record(
                    if is_ru { "Designing Data-Intensive Applications" } else { "Designing Data-Intensive Applications" },
                    Some(("status", "rd_now")),
                    1,
                );
                doc
            }
            "crm" => {
                let mut doc = Self {
                    schema_version: 1,
                    title: title.trim().to_string(),
                    pinned: false,
                    default_view: CollectionViewKind::Table,
                    group_by_column: Some("stage".to_string()),
                    columns: vec![
                        ColumnDef {
                            id: "stage".into(),
                            name: if is_ru { "Этап" } else { "Stage" }.into(),
                            col_type: ColumnType::Select,
                            width: 138.0,
                            options: vec![
                                SelectOption { id: "crm_new".into(), label: if is_ru { "Контакт" } else { "Lead" }.into(), color: ColorToken::Slate },
                                SelectOption { id: "crm_talk".into(), label: if is_ru { "Диалог" } else { "Active" }.into(), color: ColorToken::Blue },
                                SelectOption { id: "crm_won".into(), label: if is_ru { "Партнёр" } else { "Partner" }.into(), color: ColorToken::Green },
                            ],
                        },
                        ColumnDef {
                            id: "org".into(),
                            name: if is_ru { "Роль / Компания" } else { "Company" }.into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 165.0,
                        },
                        ColumnDef {
                            id: "contact".into(),
                            name: if is_ru { "Канал связи" } else { "Channel" }.into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 145.0,
                        },
                        ColumnDef {
                            id: "followup".into(),
                            name: if is_ru { "Следующий шаг" } else { "Follow-up" }.into(),
                            col_type: ColumnType::Date,
                            options: Vec::new(),
                            width: 116.0,
                        },
                    ],
                    records: Vec::new(),
                    saved_views: Vec::new(),
                };
                let _ = doc.add_record(
                    if is_ru { "Новый контакт" } else { "Sample Contact" },
                    Some(("stage", "crm_new")),
                    1,
                );
                doc
            }
            "vocabulary" => {
                let mut doc = Self {
                    schema_version: 1,
                    title: title.trim().to_string(),
                    pinned: false,
                    default_view: CollectionViewKind::Table,
                    group_by_column: Some("level".to_string()),
                    columns: vec![
                        ColumnDef {
                            id: "translation".into(),
                            name: if is_ru { "Перевод" } else { "Translation" }.into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 165.0,
                        },
                        ColumnDef {
                            id: "example".into(),
                            name: if is_ru { "Пример" } else { "Example" }.into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 240.0,
                        },
                        ColumnDef {
                            id: "level".into(),
                            name: if is_ru { "Уровень" } else { "Level" }.into(),
                            col_type: ColumnType::Select,
                            width: 136.0,
                            options: vec![
                                SelectOption {
                                    id: "lvl_new".into(),
                                    label: if is_ru { "Новое" } else { "New" }.into(),
                                    color: ColorToken::Blue,
                                },
                                SelectOption {
                                    id: "lvl_review".into(),
                                    label: if is_ru { "Повторить" } else { "Review" }.into(),
                                    color: ColorToken::Amber,
                                },
                                SelectOption {
                                    id: "lvl_done".into(),
                                    label: if is_ru { "Выучено" } else { "Mastered" }.into(),
                                    color: ColorToken::Green,
                                },
                            ],
                        },
                        ColumnDef {
                            id: "next_review".into(),
                            name: if is_ru { "Следующее повторение" } else { "Next Review" }.into(),
                            col_type: ColumnType::Date,
                            options: Vec::new(),
                            width: 155.0,
                        },
                    ],
                    records: Vec::new(),
                    saved_views: vec![
                        SavedCollectionView {
                            id: "view_due_today".into(),
                            name: if is_ru { "На сегодня" } else { "Due Today" }.into(),
                            view_kind: CollectionViewKind::Table,
                            date_preset: DateFilterPreset::DueTodayOrOverdue,
                            ..Default::default()
                        },
                        SavedCollectionView {
                            id: "view_all".into(),
                            name: if is_ru { "Все слова" } else { "All Words" }.into(),
                            view_kind: CollectionViewKind::Table,
                            date_preset: DateFilterPreset::All,
                            ..Default::default()
                        },
                    ],
                };
                let r1 = doc.add_record("ubiquitous", Some(("level", "lvl_new")), 1);
                if let Some(rec) = doc.records.iter_mut().find(|r| r.id == r1) {
                    rec.values.insert(
                        "translation".into(),
                        FieldValue::Text(if is_ru { "повсеместный" } else { "everywhere" }.into()),
                    );
                    rec.values.insert(
                        "example".into(),
                        FieldValue::Text("Smartphones have become ubiquitous.".into()),
                    );
                }
                doc
            }
            "media_library" => {
                let title_loc = if is_ru && title.eq_ignore_ascii_case("Media Library") {
                    "Кинотека".to_string()
                } else {
                    title.to_string()
                };
                let mut doc = Self {
                    schema_version: 1,
                    title: title_loc,
                    pinned: false,
                    default_view: CollectionViewKind::Gallery,
                    group_by_column: Some("status".to_string()),
                    columns: vec![
                        ColumnDef {
                            id: "media_type".into(),
                            name: if is_ru { "Тип" } else { "Type" }.into(),
                            col_type: ColumnType::Select,
                            width: 112.0,
                            options: vec![
                                SelectOption {
                                    id: "movie".into(),
                                    label: if is_ru { "Фильм" } else { "Movie" }.into(),
                                    color: ColorToken::Blue,
                                },
                                SelectOption {
                                    id: "tv".into(),
                                    label: if is_ru { "Сериал" } else { "Series" }.into(),
                                    color: ColorToken::Violet,
                                },
                            ],
                        },
                        ColumnDef {
                            id: "status".into(),
                            name: if is_ru { "Статус" } else { "Status" }.into(),
                            col_type: ColumnType::Select,
                            width: 155.0,
                            options: vec![
                                SelectOption {
                                    id: "watchlist".into(),
                                    label: if is_ru { "Хочу посмотреть" } else { "Watchlist" }.into(),
                                    color: ColorToken::Slate,
                                },
                                SelectOption {
                                    id: "watching".into(),
                                    label: if is_ru { "Смотрю" } else { "Watching" }.into(),
                                    color: ColorToken::Amber,
                                },
                                SelectOption {
                                    id: "watched".into(),
                                    label: if is_ru { "Посмотрел" } else { "Watched" }.into(),
                                    color: ColorToken::Green,
                                },
                                SelectOption {
                                    id: "dropped".into(),
                                    label: if is_ru { "Бросил" } else { "Dropped" }.into(),
                                    color: ColorToken::Rose,
                                },
                            ],
                        },
                        ColumnDef {
                            id: "rating".into(),
                            name: if is_ru { "Оценка" } else { "Rating" }.into(),
                            col_type: ColumnType::Number,
                            options: Vec::new(),
                            width: 90.0,
                        },
                        ColumnDef {
                            id: "year".into(),
                            name: if is_ru { "Год" } else { "Year" }.into(),
                            col_type: ColumnType::Number,
                            options: Vec::new(),
                            width: 84.0,
                        },
                        ColumnDef {
                            id: "genres".into(),
                            name: if is_ru { "Жанры" } else { "Genres" }.into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 170.0,
                        },
                        ColumnDef {
                            id: "director".into(),
                            name: if is_ru {
                                "Режиссёр / Создатель"
                            } else {
                                "Director / Creator"
                            }
                            .into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 180.0,
                        },
                        ColumnDef {
                            id: "progress".into(),
                            name: if is_ru { "Прогресс" } else { "Progress" }.into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 110.0,
                        },
                        ColumnDef {
                            id: "review".into(),
                            name: if is_ru { "Мини-отзыв" } else { "Mini-review" }.into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 240.0,
                        },
                        ColumnDef {
                            id: "note_link".into(),
                            name: if is_ru { "Заметка" } else { "Review Note" }.into(),
                            col_type: ColumnType::NoteLink,
                            options: Vec::new(),
                            width: 170.0,
                        },
                        ColumnDef {
                            id: "tmdb_id".into(),
                            name: "TMDB ID".into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 100.0,
                        },
                        ColumnDef {
                            id: "poster_path".into(),
                            name: if is_ru { "Постер" } else { "Poster" }.into(),
                            col_type: ColumnType::Text,
                            options: Vec::new(),
                            width: 180.0,
                        },
                    ],
                    records: Vec::new(),
                    saved_views: vec![
                        SavedCollectionView {
                            id: "view_watchlist".into(),
                            name: if is_ru { "Хочу посмотреть" } else { "Watchlist" }.into(),
                            view_kind: CollectionViewKind::Gallery,
                            select_col_id: Some("status".into()),
                            select_opt_id: Some("watchlist".into()),
                            ..Default::default()
                        },
                        SavedCollectionView {
                            id: "view_watching".into(),
                            name: if is_ru { "Смотрю" } else { "Watching" }.into(),
                            view_kind: CollectionViewKind::Gallery,
                            select_col_id: Some("status".into()),
                            select_opt_id: Some("watching".into()),
                            ..Default::default()
                        },
                        SavedCollectionView {
                            id: "view_favorites".into(),
                            name: if is_ru { "Избранное (8+)" } else { "Favorites (8+)" }.into(),
                            view_kind: CollectionViewKind::Gallery,
                            min_number_col_id: Some("rating".into()),
                            min_number: Some(8),
                            ..Default::default()
                        },
                    ],
                };
                let sample_title = if is_ru { "Интерстеллар" } else { "Interstellar" };
                let r1 = doc.add_record(sample_title, Some(("status", "watched")), 1);
                if let Some(rec) = doc.records.iter_mut().find(|r| r.id == r1) {
                    rec.values.insert("media_type".into(), FieldValue::Text("movie".into()));
                    rec.values.insert("rating".into(), FieldValue::Number(9.0));
                    rec.values.insert("year".into(), FieldValue::Number(2014.0));
                    rec.values.insert(
                        "genres".into(),
                        FieldValue::Text(
                            if is_ru { "Фантастика, Драма" } else { "Sci-Fi, Drama" }.into(),
                        ),
                    );
                    rec.values.insert(
                        "director".into(),
                        FieldValue::Text(
                            if is_ru {
                                "Кристофер Нолан"
                            } else {
                                "Christopher Nolan"
                            }
                            .into(),
                        ),
                    );
                    rec.values.insert(
                        "review".into(),
                        FieldValue::Text(if is_ru {
                            "Эталонная научная фантастика и музыка Ханса Циммера."
                        } else {
                            "Benchmark sci-fi with an unforgettable Hans Zimmer score."
                        }
                        .into()),
                    );
                }
                doc
            }
            _ => Self::new_table(title, is_ru),
        }
    }

    pub fn parse(
        raw: &str,
        fallback_title: &str,
        fallback_view: CollectionViewKind,
    ) -> Result<Self, String> {
        let mut doc: CollectionDoc =
            serde_json::from_str(raw).map_err(|e| format!("invalid collection json: {e}"))?;
        if doc.schema_version == 0 {
            doc.schema_version = 1;
        }
        if doc.title.trim().is_empty() {
            doc.title = if fallback_title.trim().is_empty() {
                "Untitled".to_string()
            } else {
                fallback_title.trim().to_string()
            };
        }
        if raw.trim().is_empty() {
            doc.default_view = fallback_view;
        }
        for rec in &mut doc.records {
            rec.tags = normalize_tags(std::mem::take(&mut rec.tags));
        }
        Ok(doc)
    }

    pub fn serialize_pretty(&self) -> Result<String, String> {
        let mut s = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialize collection json: {e}"))?;
        s = s.replace("\r\n", "\n");
        if !s.ends_with('\n') {
            s.push('\n');
        }
        Ok(s)
    }

    pub fn next_id(prefix: &str, existing: impl IntoIterator<Item = String>) -> String {
        let used: HashSet<String> = existing.into_iter().collect();
        let mut n = 1usize;
        loop {
            let cand = format!("{prefix}_{n}");
            if !used.contains(&cand) {
                return cand;
            }
            n += 1;
        }
    }

    pub fn select_columns(&self) -> Vec<&ColumnDef> {
        self.columns
            .iter()
            .filter(|c| c.col_type == ColumnType::Select)
            .collect()
    }

    pub fn effective_group_column_id(&self) -> Option<&str> {
        if let Some(ref gid) = self.group_by_column {
            if self
                .columns
                .iter()
                .any(|c| c.id == *gid && c.col_type == ColumnType::Select)
            {
                return Some(gid.as_str());
            }
        }
        self.columns
            .iter()
            .find(|c| c.col_type == ColumnType::Select)
            .map(|c| c.id.as_str())
    }

    /// Ensures a Select column exists and is set as `group_by_column`, returning its ID.
    pub fn ensure_group_column(&mut self, is_ru: bool) -> String {
        if let Some(id) = self.effective_group_column_id() {
            let out = id.to_string();
            self.group_by_column = Some(out.clone());
            return out;
        }
        let col_id = Self::next_id("col", self.columns.iter().map(|c| c.id.clone()));
        let default_opts = if is_ru {
            vec![
                SelectOption {
                    id: "opt_1".into(),
                    label: "К выполнению".into(),
                    color: ColorToken::Slate,
                },
                SelectOption {
                    id: "opt_2".into(),
                    label: "В работе".into(),
                    color: ColorToken::Blue,
                },
                SelectOption {
                    id: "opt_3".into(),
                    label: "Готово".into(),
                    color: ColorToken::Green,
                },
            ]
        } else {
            vec![
                SelectOption {
                    id: "opt_1".into(),
                    label: "To Do".into(),
                    color: ColorToken::Slate,
                },
                SelectOption {
                    id: "opt_2".into(),
                    label: "In Progress".into(),
                    color: ColorToken::Blue,
                },
                SelectOption {
                    id: "opt_3".into(),
                    label: "Done".into(),
                    color: ColorToken::Green,
                },
            ]
        };
        let first_opt = default_opts[0].id.clone();
        self.columns.insert(
            0,
            ColumnDef {
                id: col_id.clone(),
                name: if is_ru { "Статус" } else { "Status" }.to_string(),
                col_type: ColumnType::Select,
                options: default_opts,
                width: 140.0,
            },
        );
        self.group_by_column = Some(col_id.clone());
        for rec in &mut self.records {
            rec.values
                .entry(col_id.clone())
                .or_insert_with(|| FieldValue::Text(first_opt.clone()));
        }
        col_id
    }

    pub fn add_column(&mut self, name: &str, col_type: ColumnType, is_ru: bool) -> String {
        let id = Self::next_id("col", self.columns.iter().map(|c| c.id.clone()));
        let options = if col_type == ColumnType::Select {
            vec![
                SelectOption {
                    id: "opt_1".into(),
                    label: if is_ru { "Вариант 1" } else { "Option 1" }.into(),
                    color: ColorToken::Blue,
                },
                SelectOption {
                    id: "opt_2".into(),
                    label: if is_ru { "Вариант 2" } else { "Option 2" }.into(),
                    color: ColorToken::Green,
                },
            ]
        } else {
            Vec::new()
        };
        let width = match col_type {
            ColumnType::Text => 175.0,
            ColumnType::Number => 96.0,
            ColumnType::Select => 140.0,
            ColumnType::Checkbox => 78.0,
            ColumnType::Date => 114.0,
            ColumnType::NoteLink => 160.0,
        };
        self.columns.push(ColumnDef {
            id: id.clone(),
            name: name.trim().to_string(),
            col_type,
            options,
            width,
        });
        if col_type == ColumnType::Select && self.group_by_column.is_none() {
            self.group_by_column = Some(id.clone());
        }
        id
    }

    pub fn remove_column(&mut self, col_id: &str) {
        self.columns.retain(|c| c.id != col_id);
        for rec in &mut self.records {
            rec.values.remove(col_id);
        }
        if self.group_by_column.as_deref() == Some(col_id) {
            self.group_by_column = self.select_columns().first().map(|c| c.id.clone());
        }
    }

    pub fn move_column(&mut self, col_id: &str, delta: isize) {
        let Some(pos) = self.columns.iter().position(|c| c.id == col_id) else {
            return;
        };
        let next = pos as isize + delta;
        if next < 0 || next >= self.columns.len() as isize {
            return;
        }
        self.columns.swap(pos, next as usize);
    }

    pub fn add_select_option(
        &mut self,
        col_id: &str,
        label: &str,
        color: ColorToken,
    ) -> Option<String> {
        let col = self.columns.iter_mut().find(|c| c.id == col_id)?;
        let opt_id = Self::next_id("opt", col.options.iter().map(|o| o.id.clone()));
        col.options.push(SelectOption {
            id: opt_id.clone(),
            label: label.trim().to_string(),
            color,
        });
        Some(opt_id)
    }

    pub fn remove_select_option(&mut self, col_id: &str, opt_id: &str) {
        let Some(col) = self.columns.iter_mut().find(|c| c.id == col_id) else {
            return;
        };
        col.options.retain(|o| o.id != opt_id);
        let fallback = col.options.first().map(|o| o.id.clone());
        for rec in &mut self.records {
            let matches = rec
                .values
                .get(col_id)
                .is_some_and(|v| v.as_text() == opt_id);
            if matches {
                if let Some(fb) = &fallback {
                    rec.values
                        .insert(col_id.to_string(), FieldValue::Text(fb.clone()));
                } else {
                    rec.values.remove(col_id);
                }
            }
        }
    }

    pub fn move_select_option(&mut self, col_id: &str, opt_id: &str, delta: isize) {
        let Some(col) = self.columns.iter_mut().find(|c| c.id == col_id) else {
            return;
        };
        let Some(pos) = col.options.iter().position(|o| o.id == opt_id) else {
            return;
        };
        let next = pos as isize + delta;
        if next < 0 || next >= col.options.len() as isize {
            return;
        }
        col.options.swap(pos, next as usize);
    }

    pub fn add_record(
        &mut self,
        title: &str,
        group_col_opt: Option<(&str, &str)>,
        created_at: i64,
    ) -> String {
        let id = Self::next_id("rec", self.records.iter().map(|r| r.id.clone()));
        let max_order = self.records.iter().map(|r| r.order).max().unwrap_or(0);
        let mut values = BTreeMap::new();
        if let Some((col_id, opt_id)) = group_col_opt {
            values.insert(col_id.to_string(), FieldValue::Text(opt_id.to_string()));
        } else if let Some(gid) = self.effective_group_column_id() {
            if let Some(first_opt) = self
                .columns
                .iter()
                .find(|c| c.id == gid)
                .and_then(|c| c.options.first())
            {
                values.insert(gid.to_string(), FieldValue::Text(first_opt.id.clone()));
            }
        }
        self.records.push(RecordItem {
            id: id.clone(),
            title: title.trim().to_string(),
            description: String::new(),
            tags: Vec::new(),
            checklist: Vec::new(),
            values,
            order: max_order + 10,
            created_at,
        });
        id
    }

    pub fn remove_record(&mut self, record_id: &str) {
        self.records.retain(|r| r.id != record_id);
    }

    /// Moves a record to `target_opt_id` inside `group_col_id`, optionally placing it immediately
    /// before `insert_before_id`, then renumbers `order` inside the target group cleanly by steps of 10.
    pub fn move_record_to_group(
        &mut self,
        record_id: &str,
        group_col_id: &str,
        target_opt_id: &str,
        insert_before_id: Option<&str>,
    ) {
        if !self.records.iter().any(|r| r.id == record_id) {
            return;
        }
        if let Some(rec) = self.records.iter_mut().find(|r| r.id == record_id) {
            rec.values.insert(
                group_col_id.to_string(),
                FieldValue::Text(target_opt_id.to_string()),
            );
        }

        let mut target_ids: Vec<String> = {
            let mut in_group: Vec<&RecordItem> = self
                .records
                .iter()
                .filter(|r| {
                    r.id != record_id
                        && r.values
                            .get(group_col_id)
                            .is_some_and(|v| v.as_text() == target_opt_id)
                })
                .collect();
            in_group.sort_by_key(|r| (r.order, r.created_at));
            in_group.into_iter().map(|r| r.id.clone()).collect()
        };

        if let Some(before_id) = insert_before_id {
            if let Some(pos) = target_ids.iter().position(|id| id == before_id) {
                target_ids.insert(pos, record_id.to_string());
            } else {
                target_ids.push(record_id.to_string());
            }
        } else {
            target_ids.push(record_id.to_string());
        }

        for (idx, rid) in target_ids.iter().enumerate() {
            if let Some(rec) = self.records.iter_mut().find(|r| r.id == *rid) {
                rec.order = ((idx + 1) as i64) * 10;
            }
        }
    }

    /// Shifts a record up (`delta < 0`) or down (`delta > 0`) inside its current group option
    /// and renumbers group `order` cleanly in steps of 10. Returns `true` if the order changed.
    pub fn shift_record_in_group(
        &mut self,
        record_id: &str,
        group_col_id: &str,
        delta: isize,
    ) -> bool {
        if delta == 0 {
            return false;
        }
        let Some(opt_ids) = self
            .columns
            .iter()
            .find(|c| c.id == group_col_id)
            .map(|c| c.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>())
        else {
            return false;
        };
        let fallback_opt = opt_ids.first().cloned().unwrap_or_default();
        let Some(rec_raw) = self
            .records
            .iter()
            .find(|r| r.id == record_id)
            .map(|r| {
                r.values
                    .get(group_col_id)
                    .map(|v| v.as_text())
                    .unwrap_or_default()
            })
        else {
            return false;
        };
        let target_opt = if opt_ids.iter().any(|id| id == &rec_raw) {
            rec_raw
        } else {
            fallback_opt.clone()
        };
        if target_opt.is_empty() {
            return false;
        }

        let mut group_ids: Vec<String> = {
            let mut matching: Vec<&RecordItem> = self
                .records
                .iter()
                .filter(|r| {
                    let raw = r
                        .values
                        .get(group_col_id)
                        .map(|v| v.as_text())
                        .unwrap_or_default();
                    let resolved = if opt_ids.iter().any(|id| id == &raw) {
                        raw
                    } else {
                        fallback_opt.clone()
                    };
                    resolved == target_opt
                })
                .collect();
            matching.sort_by_key(|r| (r.order, r.created_at));
            matching.into_iter().map(|r| r.id.clone()).collect()
        };

        let Some(pos) = group_ids.iter().position(|id| id == record_id) else {
            return false;
        };
        let next = (pos as isize + delta).clamp(0, group_ids.len().saturating_sub(1) as isize) as usize;
        if next == pos {
            return false;
        }
        let moved = group_ids.remove(pos);
        group_ids.insert(next, moved);

        for (idx, rid) in group_ids.iter().enumerate() {
            if let Some(rec) = self.records.iter_mut().find(|r| r.id == *rid) {
                rec.values
                    .insert(group_col_id.to_string(), FieldValue::Text(target_opt.clone()));
                rec.order = ((idx + 1) as i64) * 10;
            }
        }
        true
    }

    pub fn search_text(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.title.clone());
        for col in &self.columns {
            parts.push(col.name.clone());
            for opt in &col.options {
                parts.push(opt.label.clone());
            }
        }
        for rec in &self.records {
            parts.push(rec.title.clone());
            if !rec.description.trim().is_empty() {
                parts.push(rec.description.clone());
            }
            for tag in &rec.tags {
                parts.push(tag.clone());
            }
            for item in &rec.checklist {
                parts.push(item.text.clone());
            }
            for col in &self.columns {
                if let Some(val) = rec.values.get(&col.id) {
                    if col.col_type == ColumnType::Select {
                        let raw = val.as_text();
                        if let Some(opt) = col.options.iter().find(|o| o.id == raw) {
                            parts.push(opt.label.clone());
                        }
                    } else if !val.is_empty() {
                        parts.push(val.as_text());
                    }
                }
            }
        }
        parts.join(" ")
    }

    pub fn summary_excerpt(&self, is_ru: bool) -> String {
        let n = self.records.len();
        let cols = self.columns.len();
        if n == 0 {
            if is_ru {
                format!("{cols} пол.")
            } else {
                format!("{cols} fields")
            }
        } else {
            let preview_titles: Vec<&str> = self
                .records
                .iter()
                .take(3)
                .map(|r| r.title.trim())
                .filter(|t| !t.is_empty())
                .collect();
            if is_ru {
                format!("{n} зап. · {}", preview_titles.join(", "))
            } else {
                format!("{n} items · {}", preview_titles.join(", "))
            }
        }
    }

    /// Returns `(target_raw, context)` pairs from both `NoteLink` cells and `[[wikilinks]]` in descriptions.
    pub fn extract_note_links(&self) -> Vec<(String, String)> {
        let note_link_cols: HashSet<&str> = self
            .columns
            .iter()
            .filter(|c| c.col_type == ColumnType::NoteLink)
            .map(|c| c.id.as_str())
            .collect();

        let mut out = Vec::new();
        for rec in &self.records {
            for col_id in &note_link_cols {
                if let Some(val) = rec.values.get(*col_id) {
                    let raw = val.as_text();
                    let target = strip_link_target(&raw);
                    if !target.is_empty() {
                        out.push((target.to_string(), rec.title.clone()));
                    }
                }
            }
            for wl in extract_wikilinks(&rec.description) {
                let ctx = if wl.context.trim().is_empty() {
                    rec.title.clone()
                } else {
                    wl.context
                };
                out.push((wl.target, ctx));
            }
        }
        out
    }

    /// Rewrites references inside `NoteLink` columns and `[[wikilinks]]` inside card descriptions.
    pub fn rewrite_note_links(&mut self, old_targets: &[&str], new_target: &str) -> usize {
        let clean_new = new_target.trim();
        if old_targets.is_empty() || clean_new.is_empty() {
            return 0;
        }
        let note_link_cols: Vec<String> = self
            .columns
            .iter()
            .filter(|c| c.col_type == ColumnType::NoteLink)
            .map(|c| c.id.clone())
            .collect();

        let mut total = 0usize;
        for rec in &mut self.records {
            let (new_desc, desc_count) = rewrite_wikilinks(&rec.description, old_targets, clean_new);
            if desc_count > 0 {
                rec.description = new_desc;
                total += desc_count;
            }
            for col_id in &note_link_cols {
                let Some(val) = rec.values.get(col_id).map(|v| v.as_text()) else {
                    continue;
                };
                let trimmed = val.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let inner = trimmed
                    .strip_prefix("[[")
                    .and_then(|s| s.strip_suffix("]]"));
                let (raw_target, alias) = match inner {
                    Some(inside) => match inside.split_once('|') {
                        Some((t, a)) => (t.trim(), Some(a)),
                        None => (inside.trim(), None),
                    },
                    None => match trimmed.split_once('|') {
                        Some((t, a)) => (t.trim(), Some(a)),
                        None => (trimmed, None),
                    },
                };
                let matched = old_targets
                    .iter()
                    .any(|old| !old.trim().is_empty() && raw_target.eq_ignore_ascii_case(old.trim()));
                if matched {
                    let replacement = match (inner.is_some(), alias) {
                        (true, Some(a)) => format!("[[{clean_new}|{a}]]"),
                        (true, None) => format!("[[{clean_new}]]"),
                        (false, _) => clean_new.to_string(),
                    };
                    rec.values
                        .insert(col_id.clone(), FieldValue::Text(replacement));
                    total += 1;
                }
            }
        }
        total
    }
}

pub fn strip_link_target(raw: &str) -> &str {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix("[[")
        .and_then(|s| s.strip_suffix("]]"))
        .unwrap_or(trimmed);
    inner
        .split_once('|')
        .map(|(target, _)| target.trim())
        .unwrap_or_else(|| inner.trim())
}

pub fn record_matches_smart_filter(
    rec: &RecordItem,
    cols: &[ColumnDef],
    query_lower: &str,
    date_preset: DateFilterPreset,
    tag_filter: Option<&str>,
    select_filter: Option<(&str, &str)>,
    today: SimpleDate,
) -> bool {
    if !query_lower.is_empty() {
        let mut hit = rec.title.to_lowercase().contains(query_lower)
            || rec.description.to_lowercase().contains(query_lower)
            || rec.tags.iter().any(|t| t.to_lowercase().contains(query_lower))
            || rec.checklist.iter().any(|c| c.text.to_lowercase().contains(query_lower));
        if !hit {
            for col in cols {
                if let Some(val) = rec.values.get(&col.id) {
                    if col.col_type == ColumnType::Select {
                        let raw = val.as_text();
                        if col
                            .options
                            .iter()
                            .find(|o| o.id == raw)
                            .is_some_and(|o| o.label.to_lowercase().contains(query_lower))
                        {
                            hit = true;
                            break;
                        }
                    } else if val.as_text().to_lowercase().contains(query_lower) {
                        hit = true;
                        break;
                    }
                }
            }
        }
        if !hit {
            return false;
        }
    }

    if let Some(tf) = tag_filter.filter(|s| !s.trim().is_empty()) {
        if !rec.tags.iter().any(|t| t.eq_ignore_ascii_case(tf.trim())) {
            return false;
        }
    }

    if let Some((col_id, opt_id)) = select_filter {
        let matches_opt = rec
            .values
            .get(col_id)
            .is_some_and(|v| v.as_text() == opt_id);
        if !matches_opt {
            return false;
        }
    }

    if date_preset != DateFilterPreset::All {
        let parsed_dates: Vec<SimpleDate> = cols
            .iter()
            .filter(|c| c.col_type == ColumnType::Date)
            .filter_map(|c| rec.values.get(&c.id))
            .filter_map(|v| SimpleDate::parse_iso(&v.as_text()))
            .collect();

        let matched = match date_preset {
            DateFilterPreset::All => true,
            DateFilterPreset::Overdue => parsed_dates.iter().any(|d| *d < today),
            DateFilterPreset::Today => parsed_dates.iter().any(|d| *d == today),
            DateFilterPreset::DueTodayOrOverdue => parsed_dates.iter().any(|d| *d <= today),
            DateFilterPreset::Next7Days => {
                let mut end = today;
                for _ in 0..7 {
                    end = end.next_day();
                }
                parsed_dates.iter().any(|d| *d >= today && *d <= end)
            }
            DateFilterPreset::NoDate => parsed_dates.is_empty(),
        };
        if !matched {
            return false;
        }
    }

    true
}

/// Normalizes a genre string (case/synonym-insensitive) so `Drama` and `драма` collapse to one chip.
pub fn normalize_genre_label(raw: &str, is_ru: bool) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_lowercase();
    if is_ru {
        match lower.as_str() {
            "drama" | "драма" => return "Драма".to_string(),
            "sci-fi" | "science fiction" | "фантастика" | "научная фантастика" => {
                return "Фантастика".to_string()
            }
            "thriller" | "триллер" => return "Триллер".to_string(),
            "comedy" | "комедия" => return "Комедия".to_string(),
            "action" | "боевик" => return "Боевик".to_string(),
            "crime" | "криминал" => return "Криминал".to_string(),
            "animation" | "anime" | "мультфильм" | "анимация" | "аниме" => {
                return "Анимация".to_string()
            }
            "fantasy" | "фэнтези" => return "Фэнтези".to_string(),
            "mystery" | "детектив" => return "Детектив".to_string(),
            "documentary" | "документальный" | "документальное" => {
                return "Документальное".to_string()
            }
            "horror" | "ужасы" => return "Ужасы".to_string(),
            "romance" | "мелодрама" => return "Мелодрама".to_string(),
            "adventure" | "приключения" => return "Приключения".to_string(),
            "family" | "семейный" => return "Семейный".to_string(),
            "biography" | "биография" => return "Биография".to_string(),
            "history" | "история" | "исторический" => return "История".to_string(),
            _ => {}
        }
    } else {
        match lower.as_str() {
            "drama" | "драма" => return "Drama".to_string(),
            "sci-fi" | "science fiction" | "фантастика" | "научная фантастика" => {
                return "Sci-Fi".to_string()
            }
            "thriller" | "триллер" => return "Thriller".to_string(),
            "comedy" | "комедия" => return "Comedy".to_string(),
            "action" | "боевик" => return "Action".to_string(),
            "crime" | "криминал" => return "Crime".to_string(),
            "animation" | "мультфильм" | "анимация" => return "Animation".to_string(),
            "fantasy" | "фэнтези" => return "Fantasy".to_string(),
            "mystery" | "детектив" => return "Mystery".to_string(),
            "documentary" | "документальный" | "документальное" => {
                return "Documentary".to_string()
            }
            "horror" | "ужасы" => return "Horror".to_string(),
            "romance" | "мелодрама" => return "Romance".to_string(),
            "adventure" | "приключения" => return "Adventure".to_string(),
            "family" | "семейный" => return "Family".to_string(),
            "biography" | "биография" => return "Biography".to_string(),
            "history" | "история" | "исторический" => return "History".to_string(),
            _ => {}
        }
    }
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => {
            let mut out: String = first.to_uppercase().collect();
            out.push_str(chars.as_str());
            out
        }
        None => String::new(),
    }
}

impl CollectionDoc {
    /// Localizes legacy English labels/saved views and normalizes genres in a Media Library doc in-place.
    pub fn localize_media_library_in_place(&mut self, is_ru: bool) -> bool {
        let is_media = self
            .columns
            .iter()
            .any(|c| c.id == "tmdb_id" || c.id == "poster_path");
        if !is_media {
            return false;
        }
        let mut changed = false;
        let expected_title = if is_ru { "Кинотека" } else { "Media Library" };
        let alt_title = if is_ru { "Media Library" } else { "Кинотека" };
        if self.title == alt_title {
            self.title = expected_title.to_string();
            changed = true;
        }
        for col in &mut self.columns {
            let target_name = if is_ru {
                match col.id.as_str() {
                    "media_type" => Some("Тип"),
                    "status" => Some("Статус"),
                    "rating" => Some("Оценка"),
                    "year" => Some("Год"),
                    "genres" => Some("Жанры"),
                    "director" => Some("Режиссёр / Создатель"),
                    "progress" => Some("Прогресс"),
                    "review" => Some("Мини-отзыв"),
                    "note_link" => Some("Заметка"),
                    "poster_path" => Some("Постер"),
                    _ => None,
                }
            } else {
                match col.id.as_str() {
                    "media_type" => Some("Type"),
                    "status" => Some("Status"),
                    "rating" => Some("Rating"),
                    "year" => Some("Year"),
                    "genres" => Some("Genres"),
                    "director" => Some("Director / Creator"),
                    "progress" => Some("Progress"),
                    "review" => Some("Mini-review"),
                    "note_link" => Some("Review Note"),
                    "poster_path" => Some("Poster"),
                    _ => None,
                }
            };
            if let Some(name) = target_name {
                if col.name != name {
                    col.name = name.to_string();
                    changed = true;
                }
            }
            if col.id == "media_type" || col.id == "status" {
                for opt in &mut col.options {
                    let target_opt = if is_ru {
                        match opt.id.as_str() {
                            "movie" => Some("Фильм"),
                            "tv" => Some("Сериал"),
                            "watchlist" => Some("Хочу посмотреть"),
                            "watching" => Some("Смотрю"),
                            "watched" => Some("Посмотрел"),
                            "dropped" => Some("Бросил"),
                            _ => None,
                        }
                    } else {
                        match opt.id.as_str() {
                            "movie" => Some("Movie"),
                            "tv" => Some("Series"),
                            "watchlist" => Some("Watchlist"),
                            "watching" => Some("Watching"),
                            "watched" => Some("Watched"),
                            "dropped" => Some("Dropped"),
                            _ => None,
                        }
                    };
                    if let Some(lbl) = target_opt {
                        if opt.label != lbl {
                            opt.label = lbl.to_string();
                            changed = true;
                        }
                    }
                }
            }
        }
        for sv in &mut self.saved_views {
            let target_sv = if is_ru {
                match sv.id.as_str() {
                    "view_watchlist" => Some("Хочу посмотреть"),
                    "view_watching" => Some("Смотрю"),
                    "view_favorites" => Some("Избранное (8+)"),
                    _ => None,
                }
            } else {
                match sv.id.as_str() {
                    "view_watchlist" => Some("Watchlist"),
                    "view_watching" => Some("Watching"),
                    "view_favorites" => Some("Favorites (8+)"),
                    _ => None,
                }
            };
            if let Some(name) = target_sv {
                if sv.name != name {
                    sv.name = name.to_string();
                    changed = true;
                }
            }
        }
        for rec in &mut self.records {
            if let Some(FieldValue::Text(raw_genres)) = rec.values.get("genres").cloned() {
                let mut norm_list: Vec<String> = Vec::new();
                for part in raw_genres.split([',', ';', '/']) {
                    let norm = normalize_genre_label(part, is_ru);
                    if !norm.is_empty()
                        && !norm_list.iter().any(|x| x.to_lowercase() == norm.to_lowercase())
                    {
                        norm_list.push(norm);
                    }
                }
                let joined = norm_list.join(", ");
                if joined != raw_genres && !joined.is_empty() {
                    rec.values.insert("genres".into(), FieldValue::Text(joined));
                    changed = true;
                }
            }
        }
        changed
    }
}

/// Checks an optional numeric lower-bound filter (`col_id >= min_val`), used by Saved Views such as "Favorites (8+)".
pub fn record_matches_min_number(rec: &RecordItem, min_filter: Option<(&str, i32)>) -> bool {
    let Some((col_id, min_val)) = min_filter else {
        return true;
    };
    rec.values
        .get(col_id)
        .and_then(FieldValue::as_number)
        .is_some_and(|n| n >= min_val as f64)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Compares two collection records by `__title` or a column ID, always placing
/// missing/empty field values last regardless of sort direction.
pub fn compare_records(
    a: &RecordItem,
    b: &RecordItem,
    column_key: &str,
    dir: SortDirection,
    columns: &[ColumnDef],
) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let ord = if column_key == "__title" {
        let ta = a.title.trim().to_lowercase();
        let tb = b.title.trim().to_lowercase();
        match (ta.is_empty(), tb.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => return Ordering::Greater,
            (false, true) => return Ordering::Less,
            (false, false) => ta.cmp(&tb),
        }
    } else if let Some(col) = columns.iter().find(|c| c.id == column_key) {
        let va = a.values.get(column_key);
        let vb = b.values.get(column_key);
        match col.col_type {
            ColumnType::Select => {
                let ia = va
                    .map(FieldValue::as_text)
                    .filter(|s| !s.trim().is_empty())
                    .and_then(|id| col.options.iter().position(|o| o.id == id));
                let ib = vb
                    .map(FieldValue::as_text)
                    .filter(|s| !s.trim().is_empty())
                    .and_then(|id| col.options.iter().position(|o| o.id == id));
                match (ia, ib) {
                    (None, None) => Ordering::Equal,
                    (None, Some(_)) => return Ordering::Greater,
                    (Some(_), None) => return Ordering::Less,
                    (Some(x), Some(y)) => x.cmp(&y),
                }
            }
            ColumnType::Date => {
                let da = va
                    .map(FieldValue::as_text)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let db = vb
                    .map(FieldValue::as_text)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                match (da, db) {
                    (None, None) => Ordering::Equal,
                    (None, Some(_)) => return Ordering::Greater,
                    (Some(_), None) => return Ordering::Less,
                    (Some(sa), Some(sb)) => {
                        match (SimpleDate::parse_iso(&sa), SimpleDate::parse_iso(&sb)) {
                            (Some(pa), Some(pb)) => pa.cmp(&pb),
                            _ => sa.cmp(&sb),
                        }
                    }
                }
            }
            ColumnType::Number => {
                let na = va.and_then(FieldValue::as_number);
                let nb = vb.and_then(FieldValue::as_number);
                match (na, nb) {
                    (None, None) => Ordering::Equal,
                    (None, Some(_)) => return Ordering::Greater,
                    (Some(_), None) => return Ordering::Less,
                    (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
                }
            }
            ColumnType::Checkbox => {
                let ba = va.map(FieldValue::as_bool).unwrap_or(false);
                let bb = vb.map(FieldValue::as_bool).unwrap_or(false);
                ba.cmp(&bb)
            }
            _ => {
                let sa = va.map(FieldValue::as_text).unwrap_or_default().trim().to_lowercase();
                let sb = vb.map(FieldValue::as_text).unwrap_or_default().trim().to_lowercase();
                match (sa.is_empty(), sb.is_empty()) {
                    (true, true) => Ordering::Equal,
                    (true, false) => return Ordering::Greater,
                    (false, true) => return Ordering::Less,
                    (false, false) => sa.cmp(&sb),
                }
            }
        }
    } else {
        Ordering::Equal
    };

    let directed = match dir {
        SortDirection::Asc => ord,
        SortDirection::Desc => ord.reverse(),
    };
    directed.then_with(|| (a.order, a.created_at).cmp(&(b.order, b.created_at)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_and_table_json_roundtrip_is_deterministic() {
        let mut board = CollectionDoc::new_board("Контент-план YouTube", true);
        let r1 = board.add_record("Выпуск 1: Rust GUI", Some(("stage", "opt_idea")), 100);
        let r2 = board.add_record("Выпуск 2: SQLite Sync", Some(("stage", "opt_work")), 200);
        board.records[0].description = "Опора на [[Architecture]].".into();
        board.records[0]
            .values
            .insert("note_link".into(), FieldValue::Text("Architecture".into()));
        board.records[1].checklist.push(ChecklistItem {
            text: "Снять интро".into(),
            done: true,
        });

        let json1 = board.serialize_pretty().unwrap();
        let parsed =
            CollectionDoc::parse(&json1, "Fallback", CollectionViewKind::Board).unwrap();
        let json2 = parsed.serialize_pretty().unwrap();
        assert_eq!(json1, json2);
        assert_eq!(parsed.records.len(), 2);
        assert_eq!(parsed.records[0].id, r1);
        assert_eq!(parsed.records[1].id, r2);
    }

    #[test]
    fn move_record_reorders_cleanly_and_updates_links() {
        let mut board = CollectionDoc::new_board("Sprint", false);
        let a = board.add_record("Task A", Some(("stage", "opt_idea")), 10);
        let b = board.add_record("Task B", Some(("stage", "opt_work")), 20);
        let c = board.add_record("Task C", Some(("stage", "opt_work")), 30);

        board.move_record_to_group(&a, "stage", "opt_work", Some(&b));
        let mut work_cards: Vec<&RecordItem> = board
            .records
            .iter()
            .filter(|r| r.values.get("stage").map(FieldValue::as_text).as_deref() == Some("opt_work"))
            .collect();
        work_cards.sort_by_key(|r| r.order);
        let order_ids: Vec<&str> = work_cards.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(order_ids, vec![a.as_str(), b.as_str(), c.as_str()]);

        if let Some(rec) = board.records.iter_mut().find(|r| r.id == b) {
            rec.description = "See [[Old Spec]] and [[Old Spec|Spec]].".into();
            rec.values
                .insert("note_link".into(), FieldValue::Text("Old Spec".into()));
        }
        let links = board.extract_note_links();
        assert!(links.iter().any(|(t, _)| t == "Old Spec"));

        let rewritten = board.rewrite_note_links(&["Old Spec", "old-spec"], "New Spec");
        assert_eq!(rewritten, 3);
        let b_rec = board.records.iter().find(|r| r.id == b).unwrap();
        assert_eq!(b_rec.description, "See [[New Spec]] and [[New Spec|Spec]].");
        assert_eq!(
            b_rec.values.get("note_link").unwrap().as_text(),
            "New Spec"
        );
    }

    #[test]
    fn ensure_group_column_upgrades_selectless_table() {
        let mut table = CollectionDoc::new_table("Expenses", false);
        table.remove_column("status");
        assert!(table.effective_group_column_id().is_none());
        let rec_id = table.add_record("Rent", None, 50);
        let col_id = table.ensure_group_column(false);
        assert_eq!(table.effective_group_column_id(), Some(col_id.as_str()));
        let rec = table.records.iter().find(|r| r.id == rec_id).unwrap();
        assert!(rec.values.contains_key(&col_id));
    }

    #[test]
    fn builds_curated_board_and_table_presets() {
        for pid in ["kanban", "content", "sprint", "blank"] {
            let b = CollectionDoc::new_board_preset("Board", pid, true);
            assert_eq!(b.default_view, CollectionViewKind::Board);
            assert!(b.effective_group_column_id().is_some());
            assert!(!b.serialize_pretty().unwrap().is_empty());
        }
        for pid in ["general", "subscriptions", "reading", "crm"] {
            let t = CollectionDoc::new_table_preset("Table", pid, false);
            assert_eq!(t.default_view, CollectionViewKind::Table);
            assert!(t.columns.len() >= 4);
            assert!(!t.serialize_pretty().unwrap().is_empty());
        }
    }

    #[test]
    fn shifts_record_up_and_down_within_same_group() {
        let mut board = CollectionDoc::new_board("Sprint", false);
        let a = board.add_record("Task A", Some(("stage", "opt_idea")), 10);
        let b = board.add_record("Task B", Some(("stage", "opt_idea")), 20);
        let c = board.add_record("Task C", Some(("stage", "opt_idea")), 30);

        // Moving first item up or last item down is a no-op
        assert!(!board.shift_record_in_group(&a, "stage", -1));
        assert!(!board.shift_record_in_group(&c, "stage", 1));

        // Move C up once -> [A, C, B]
        assert!(board.shift_record_in_group(&c, "stage", -1));
        let ordered = |doc: &CollectionDoc| -> Vec<String> {
            let mut v: Vec<&RecordItem> = doc.records.iter().collect();
            v.sort_by_key(|r| (r.order, r.created_at));
            v.into_iter().map(|r| r.id.clone()).collect()
        };
        assert_eq!(ordered(&board), vec![a.clone(), c.clone(), b.clone()]);

        // Move A down once -> [C, A, B]
        assert!(board.shift_record_in_group(&a, "stage", 1));
        assert_eq!(ordered(&board), vec![c, a, b]);
    }

    #[test]
    fn filters_records_by_date_presets_tags_and_saved_views() {
        let mut board = CollectionDoc::new_board("Sprint", true);
        let r_over = board.add_record("Overdue item", None, 1);
        let r_today = board.add_record("Today item", None, 2);
        let r_week = board.add_record("Week item", None, 3);
        let r_nodate = board.add_record("No date item", None, 4);

        let today = SimpleDate::from_ymd(2026, 11, 20).unwrap();
        for (rid, date_str, prio, tag) in [
            (&r_over, Some("2026-11-18"), "prio_high", "ops"),
            (&r_today, Some("2026-11-20"), "prio_med", "ui"),
            (&r_week, Some("2026-11-25"), "prio_low", "ui"),
            (&r_nodate, None, "prio_low", "ops"),
        ] {
            let rec = board.records.iter_mut().find(|r| r.id == *rid).unwrap();
            rec.values
                .insert("priority".into(), FieldValue::Text(prio.into()));
            rec.tags.push(tag.into());
            if let Some(ds) = date_str {
                rec.values.insert("due".into(), FieldValue::Text(ds.into()));
            }
        }
        board.saved_views.push(SavedCollectionView {
            id: "v1".into(),
            name: "Срочные UI".into(),
            view_kind: CollectionViewKind::Board,
            date_preset: DateFilterPreset::Next7Days,
            tag: Some("ui".into()),
            ..Default::default()
        });

        let json = board.serialize_pretty().unwrap();
        let reparsed = CollectionDoc::parse(&json, "Sprint", CollectionViewKind::Board).unwrap();
        assert_eq!(reparsed.saved_views.len(), 1);
        assert_eq!(reparsed.saved_views[0].name, "Срочные UI");

        let count_matching = |preset: DateFilterPreset, tag: Option<&str>, sel: Option<(&str, &str)>| {
            reparsed
                .records
                .iter()
                .filter(|r| {
                    record_matches_smart_filter(r, &reparsed.columns, "", preset, tag, sel, today)
                })
                .count()
        };
        assert_eq!(count_matching(DateFilterPreset::Overdue, None, None), 1);
        assert_eq!(count_matching(DateFilterPreset::Today, None, None), 1);
        assert_eq!(count_matching(DateFilterPreset::Next7Days, None, None), 2);
        assert_eq!(count_matching(DateFilterPreset::NoDate, None, None), 1);
        assert_eq!(
            count_matching(DateFilterPreset::Next7Days, Some("ui"), None),
            2
        );
        assert_eq!(
            count_matching(DateFilterPreset::All, None, Some(("priority", "prio_high"))),
            1
        );
    }

    #[test]
    fn sorts_records_by_priority_date_and_title_with_empty_last() {
        let board = CollectionDoc::new_board("Test", true);
        let mut r_low = RecordItem { id: "rec_1".into(), title: "Beta".into(), ..Default::default() };
        r_low.values.insert("priority".into(), FieldValue::Text("prio_low".into()));
        r_low.values.insert("due".into(), FieldValue::Text("2026-11-30".into()));

        let mut r_high = RecordItem { id: "rec_2".into(), title: "Alpha".into(), ..Default::default() };
        r_high.values.insert("priority".into(), FieldValue::Text("prio_high".into()));

        let mut r_none = RecordItem { id: "rec_3".into(), title: "Gamma".into(), ..Default::default() };
        r_none.values.insert("due".into(), FieldValue::Text("2026-11-20".into()));

        let mut list = vec![r_low.clone(), r_none.clone(), r_high.clone()];
        list.sort_by(|a, b| compare_records(a, b, "priority", SortDirection::Asc, &board.columns));
        assert_eq!(list.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec!["rec_2", "rec_1", "rec_3"]);

        list.sort_by(|a, b| compare_records(a, b, "due", SortDirection::Asc, &board.columns));
        assert_eq!(list.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec!["rec_3", "rec_1", "rec_2"]);

        list.sort_by(|a, b| compare_records(a, b, "due", SortDirection::Desc, &board.columns));
        assert_eq!(list.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec!["rec_1", "rec_3", "rec_2"]);
    }

    #[test]
    fn vocabulary_preset_and_dynamic_due_filter() {
        let mut doc = CollectionDoc::new_table_preset("English Vocab", "vocabulary", true);
        assert_eq!(doc.saved_views.len(), 2);
        assert_eq!(doc.saved_views[0].date_preset, DateFilterPreset::DueTodayOrOverdue);

        let r_due = doc.add_record("resilient", Some(("level", "lvl_review")), 2);
        if let Some(rec) = doc.records.iter_mut().find(|r| r.id == r_due) {
            rec.values.insert("next_review".into(), FieldValue::Text("2026-11-20".into()));
        }
        let r_today = doc.add_record("ephemeral", Some(("level", "lvl_new")), 3);
        if let Some(rec) = doc.records.iter_mut().find(|r| r.id == r_today) {
            rec.values.insert("next_review".into(), FieldValue::Text("2026-11-21".into()));
        }
        let r_future = doc.add_record("pragmatic", Some(("level", "lvl_done")), 4);
        if let Some(rec) = doc.records.iter_mut().find(|r| r.id == r_future) {
            rec.values.insert("next_review".into(), FieldValue::Text("2026-11-25".into()));
        }

        let today = SimpleDate::from_ymd(2026, 11, 21).unwrap();
        let due_titles: Vec<&str> = doc
            .records
            .iter()
            .filter(|r| {
                record_matches_smart_filter(
                    r,
                    &doc.columns,
                    "",
                    DateFilterPreset::DueTodayOrOverdue,
                    None,
                    None,
                    today,
                )
            })
            .map(|r| r.title.as_str())
            .collect();
        assert_eq!(due_titles, vec!["resilient", "ephemeral"]);
    }

    #[test]
    fn gallery_view_and_min_number_filter_roundtrip() {
        let legacy_json = r#"{"id":"v1","name":"Old","view_kind":"board"}"#;
        let legacy: SavedCollectionView = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(legacy.view_kind, CollectionViewKind::Board);
        assert_eq!(legacy.min_number_col_id, None);
        assert_eq!(legacy.min_number, None);

        let fav = SavedCollectionView {
            id: "fav".into(),
            name: "8+".into(),
            view_kind: CollectionViewKind::Gallery,
            min_number_col_id: Some("rating".into()),
            min_number: Some(8),
            ..Default::default()
        };
        let encoded = serde_json::to_string(&fav).unwrap();
        assert!(encoded.contains("\"gallery\""));
        let decoded: SavedCollectionView = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, fav);

        let mut r_ok = RecordItem { id: "1".into(), title: "A".into(), ..Default::default() };
        r_ok.values.insert("rating".into(), FieldValue::Number(8.0));
        let mut r_low = RecordItem { id: "2".into(), title: "B".into(), ..Default::default() };
        r_low.values.insert("rating".into(), FieldValue::Number(7.9));
        let r_empty = RecordItem { id: "3".into(), title: "C".into(), ..Default::default() };

        assert!(record_matches_min_number(&r_ok, Some(("rating", 8))));
        assert!(!record_matches_min_number(&r_low, Some(("rating", 8))));
        assert!(!record_matches_min_number(&r_empty, Some(("rating", 8))));
        assert!(record_matches_min_number(&r_empty, None));
    }

    #[test]
    fn media_library_preset_schema_and_views() {
        let mut doc = CollectionDoc::new_table_preset("Кинотека", "media_library", true);
        assert_eq!(doc.default_view, CollectionViewKind::Gallery);
        assert_eq!(doc.columns.len(), 11);
        assert_eq!(doc.saved_views.len(), 3);
        let fav = &doc.saved_views[2];
        assert_eq!(fav.min_number_col_id.as_deref(), Some("rating"));
        assert_eq!(fav.min_number, Some(8));

        let r_low = doc.add_record("Film Low", Some(("status", "watched")), 2);
        if let Some(rec) = doc.records.iter_mut().find(|r| r.id == r_low) {
            rec.values.insert("rating".into(), FieldValue::Number(7.0));
        }
        let matches: Vec<&str> = doc
            .records
            .iter()
            .filter(|r| record_matches_min_number(r, Some(("rating", 8))))
            .map(|r| r.title.as_str())
            .collect();
        assert_eq!(matches, vec!["Интерстеллар"]);
    }

    #[test]
    fn normalizes_genres_and_localizes_media_library() {
        assert_eq!(normalize_genre_label("Drama", true), "Драма");
        assert_eq!(normalize_genre_label(" драма ", true), "Драма");
        assert_eq!(normalize_genre_label("Sci-Fi", true), "Фантастика");
        assert_eq!(normalize_genre_label("Драма", false), "Drama");

        let mut doc = CollectionDoc::new_table_preset("Media Library", "media_library", false);
        if let Some(rec) = doc.records.first_mut() {
            rec.values
                .insert("genres".into(), FieldValue::Text("Drama, драма, Sci-Fi".into()));
        }
        assert!(doc.localize_media_library_in_place(true));
        assert_eq!(doc.title, "Кинотека");
        assert_eq!(doc.saved_views[0].name, "Хочу посмотреть");
        assert_eq!(doc.saved_views[2].name, "Избранное (8+)");
        assert_eq!(
            doc.records[0].values.get("genres").map(FieldValue::as_text).as_deref(),
            Some("Драма, Фантастика")
        );

        assert!(doc.localize_media_library_in_place(false));
        assert_eq!(doc.title, "Media Library");
        assert_eq!(doc.saved_views[0].name, "Watchlist");
        assert_eq!(doc.saved_views[2].name, "Favorites (8+)");
        assert_eq!(
            doc.records[0].values.get("genres").map(FieldValue::as_text).as_deref(),
            Some("Drama, Sci-Fi")
        );
    }
}
