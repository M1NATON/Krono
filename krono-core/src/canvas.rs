use crate::markdown::{extract_wikilinks, plain_excerpt, rewrite_wikilinks};
use crate::path::vault_file_stem;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CanvasNodeKind {
    #[default]
    Text,
    File,
    Link,
    Group,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CanvasSide {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CanvasEnd {
    #[default]
    None,
    Arrow,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasNode {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: CanvasNodeKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subpath: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Default for CanvasNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: CanvasNodeKind::Text,
            x: 0.0,
            y: 0.0,
            width: 240.0,
            height: 120.0,
            color: None,
            text: None,
            file: None,
            subpath: None,
            url: None,
            label: None,
        }
    }
}

impl CanvasNode {
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width * 0.5, self.y + self.height * 0.5)
    }

    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    pub fn display_title(&self) -> String {
        match self.kind {
            CanvasNodeKind::Text => self
                .text
                .as_deref()
                .and_then(|t| t.lines().map(str::trim).find(|l| !l.is_empty()))
                .map(|l| l.trim_start_matches('#').trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Card".to_string()),
            CanvasNodeKind::File => self
                .file
                .as_deref()
                .map(|p| vault_file_stem(p).to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "File".to_string()),
            CanvasNodeKind::Link => self
                .url
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Link".to_string()),
            CanvasNodeKind::Group => self
                .label
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Group".to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasEdge {
    pub id: String,
    #[serde(rename = "fromNode")]
    pub from_node: String,
    #[serde(rename = "fromSide", default, skip_serializing_if = "Option::is_none")]
    pub from_side: Option<CanvasSide>,
    #[serde(rename = "fromEnd", default, skip_serializing_if = "Option::is_none")]
    pub from_end: Option<CanvasEnd>,
    #[serde(rename = "toNode")]
    pub to_node: String,
    #[serde(rename = "toSide", default, skip_serializing_if = "Option::is_none")]
    pub to_side: Option<CanvasSide>,
    #[serde(rename = "toEnd", default, skip_serializing_if = "Option::is_none")]
    pub to_end: Option<CanvasEnd>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Default for CanvasEdge {
    fn default() -> Self {
        Self {
            id: String::new(),
            from_node: String::new(),
            from_side: None,
            from_end: None,
            to_node: String::new(),
            to_side: None,
            to_end: Some(CanvasEnd::Arrow),
            color: None,
            label: None,
        }
    }
}

/// JSON Canvas 1.0 compatible document stored in `*.canvas`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasDoc {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pinned: bool,
    #[serde(default)]
    pub nodes: Vec<CanvasNode>,
    #[serde(default)]
    pub edges: Vec<CanvasEdge>,
}

impl CanvasDoc {
    pub fn new_storyboard(title: &str, is_ru: bool) -> Self {
        let mut doc = Self {
            title: title.trim().to_string(),
            pinned: false,
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        let grp_label = if is_ru { "01. Начало" } else { "01. Start" };
        let card_text = if is_ru {
            "### Главная идея\nОпишите ключевой тезис и протяните стрелку к следующей карточке."
        } else {
            "### Core Idea\nWrite the opening hook and drag from a side handle to connect cards."
        };
        doc.add_group_node(-20.0, -40.0, 320.0, 230.0, grp_label, Some("6"));
        doc.add_text_node(16.0, 20.0, card_text, Some("5"));
        doc
    }

    /// Creates a canvas from a curated preset (`"storyboard"`, `"mindmap"`, `"architecture"`, `"blank"`).
    pub fn new_preset(title: &str, preset_id: &str, is_ru: bool) -> Self {
        match preset_id {
            "blank" => Self {
                title: title.trim().to_string(),
                pinned: false,
                nodes: Vec::new(),
                edges: Vec::new(),
            },
            "mindmap" => {
                let mut doc = Self {
                    title: title.trim().to_string(),
                    pinned: false,
                    nodes: Vec::new(),
                    edges: Vec::new(),
                };
                let center = doc.add_text_node(
                    0.0,
                    0.0,
                    if is_ru {
                        "### Центральная тема\nГлавная цель или проблема"
                    } else {
                        "### Central Topic\nCore objective or question"
                    },
                    Some("6"),
                );
                let left = doc.add_text_node(
                    -320.0,
                    -100.0,
                    if is_ru { "### Контекст\n- Факты\n- Ограничения" } else { "### Context\n- Facts\n- Constraints" },
                    Some("5"),
                );
                let top = doc.add_text_node(
                    0.0,
                    -200.0,
                    if is_ru { "### Гипотезы\n- Вариант А\n- Вариант Б" } else { "### Ideas\n- Option A\n- Option B" },
                    Some("3"),
                );
                let right = doc.add_text_node(
                    320.0,
                    -100.0,
                    if is_ru { "### Действия\n- [ ] Первый шаг\n- [ ] Проверить" } else { "### Next Steps\n- [ ] First step\n- [ ] Validate" },
                    Some("4"),
                );
                let bottom = doc.add_text_node(
                    0.0,
                    200.0,
                    if is_ru { "### Риски\nЧто может помешать?" } else { "### Risks\nWhat could go wrong?" },
                    Some("1"),
                );
                doc.add_edge(&center, Some(CanvasSide::Left), &left, Some(CanvasSide::Right), None);
                doc.add_edge(&center, Some(CanvasSide::Top), &top, Some(CanvasSide::Bottom), None);
                doc.add_edge(&center, Some(CanvasSide::Right), &right, Some(CanvasSide::Left), None);
                doc.add_edge(&center, Some(CanvasSide::Bottom), &bottom, Some(CanvasSide::Top), None);
                doc
            }
            "architecture" => {
                let mut doc = Self {
                    title: title.trim().to_string(),
                    pinned: false,
                    nodes: Vec::new(),
                    edges: Vec::new(),
                };
                doc.add_group_node(-340.0, -60.0, 290.0, 220.0, if is_ru { "Клиент" } else { "Client" }, Some("5"));
                doc.add_group_node(-10.0, -60.0, 290.0, 220.0, if is_ru { "Логика" } else { "Core Logic" }, Some("6"));
                doc.add_group_node(320.0, -60.0, 290.0, 220.0, if is_ru { "Данные" } else { "Storage" }, Some("4"));
                let n1 = doc.add_text_node(
                    -320.0,
                    0.0,
                    if is_ru { "### UI / Ввод\nСобытия пользователя" } else { "### UI Layer\nUser interactions" },
                    Some("5"),
                );
                let n2 = doc.add_text_node(
                    10.0,
                    0.0,
                    if is_ru { "### Сервис\nОбработка и правила" } else { "### Service\nBusiness rules" },
                    Some("6"),
                );
                let n3 = doc.add_text_node(
                    340.0,
                    0.0,
                    if is_ru { "### База / API\nХранение и синк" } else { "### Store / API\nPersistence & sync" },
                    Some("4"),
                );
                doc.add_edge(&n1, Some(CanvasSide::Right), &n2, Some(CanvasSide::Left), Some(if is_ru { "запрос" } else { "request" }));
                doc.add_edge(&n2, Some(CanvasSide::Right), &n3, Some(CanvasSide::Left), Some(if is_ru { "чтение/запись" } else { "r/w" }));
                doc
            }
            _ => {
                // Full 3-stage Video Storyboard
                let mut doc = Self {
                    title: title.trim().to_string(),
                    pinned: false,
                    nodes: Vec::new(),
                    edges: Vec::new(),
                };
                doc.add_group_node(-340.0, -40.0, 280.0, 220.0, if is_ru { "01. Вступление" } else { "01. Hook" }, Some("6"));
                doc.add_group_node(-20.0, -40.0, 280.0, 220.0, if is_ru { "02. Основная часть" } else { "02. Body" }, Some("5"));
                doc.add_group_node(300.0, -40.0, 280.0, 220.0, if is_ru { "03. Финал" } else { "03. Outro" }, Some("4"));
                let c1 = doc.add_text_node(
                    -320.0,
                    15.0,
                    if is_ru { "### Хук (0:00–0:20)\nЗацепите внимание зрителя." } else { "### Opening Hook\nGrab attention in 15s." },
                    Some("5"),
                );
                let c2 = doc.add_text_node(
                    0.0,
                    15.0,
                    if is_ru { "### Ключевые тезисы\n1. Проблема\n2. Демонстрация" } else { "### Main Points\n1. Problem\n2. Walkthrough" },
                    Some("3"),
                );
                let c3 = doc.add_text_node(
                    320.0,
                    15.0,
                    if is_ru { "### Итог и CTA\nГлавный вывод ролика." } else { "### Takeaways & CTA\nWrap up clearly." },
                    Some("4"),
                );
                doc.add_edge(&c1, Some(CanvasSide::Right), &c2, Some(CanvasSide::Left), None);
                doc.add_edge(&c2, Some(CanvasSide::Right), &c3, Some(CanvasSide::Left), None);
                doc
            }
        }
    }

    pub fn parse(raw: &str, fallback_title: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        let mut doc: CanvasDoc = if trimmed.is_empty() {
            CanvasDoc::default()
        } else {
            serde_json::from_str(trimmed).map_err(|e| format!("parse canvas json: {e}"))?
        };
        if doc.title.trim().is_empty() {
            doc.title = fallback_title.trim().to_string();
        }
        Ok(doc)
    }

    pub fn serialize_pretty(&self) -> Result<String, String> {
        let mut json =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize canvas json: {e}"))?;
        json = json.replace("\r\n", "\n");
        if !json.ends_with('\n') {
            json.push('\n');
        }
        Ok(json)
    }

    fn next_id(prefix: &str, existing: impl Iterator<Item = String>) -> String {
        let used: HashSet<String> = existing.collect();
        let mut idx = 1usize;
        loop {
            let candidate = format!("{prefix}_{idx}");
            if !used.contains(&candidate) {
                return candidate;
            }
            idx += 1;
        }
    }

    pub fn add_text_node(
        &mut self,
        x: f32,
        y: f32,
        text: &str,
        color: Option<&str>,
    ) -> String {
        let id = Self::next_id("n", self.nodes.iter().map(|n| n.id.clone()));
        self.nodes.push(CanvasNode {
            id: id.clone(),
            kind: CanvasNodeKind::Text,
            x,
            y,
            width: 248.0,
            height: 132.0,
            color: color.map(str::to_string),
            text: Some(text.to_string()),
            ..CanvasNode::default()
        });
        id
    }

    pub fn add_file_node(
        &mut self,
        x: f32,
        y: f32,
        file_path: &str,
        color: Option<&str>,
    ) -> String {
        let id = Self::next_id("n", self.nodes.iter().map(|n| n.id.clone()));
        self.nodes.push(CanvasNode {
            id: id.clone(),
            kind: CanvasNodeKind::File,
            x,
            y,
            width: 240.0,
            height: 112.0,
            color: color.map(str::to_string),
            file: Some(file_path.to_string()),
            ..CanvasNode::default()
        });
        id
    }

    pub fn add_group_node(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        label: &str,
        color: Option<&str>,
    ) -> String {
        let id = Self::next_id("g", self.nodes.iter().map(|n| n.id.clone()));
        self.nodes.push(CanvasNode {
            id: id.clone(),
            kind: CanvasNodeKind::Group,
            x,
            y,
            width: width.max(160.0),
            height: height.max(120.0),
            color: color.map(str::to_string),
            label: Some(label.to_string()),
            ..CanvasNode::default()
        });
        id
    }

    pub fn remove_node(&mut self, node_id: &str) {
        self.nodes.retain(|n| n.id != node_id);
        self.edges
            .retain(|e| e.from_node != node_id && e.to_node != node_id);
    }

    pub fn add_edge(
        &mut self,
        from_node: &str,
        from_side: Option<CanvasSide>,
        to_node: &str,
        to_side: Option<CanvasSide>,
        label: Option<&str>,
    ) -> Option<String> {
        if from_node == to_node {
            return None;
        }
        let has_from = self.nodes.iter().any(|n| n.id == from_node);
        let has_to = self.nodes.iter().any(|n| n.id == to_node);
        if !has_from || !has_to {
            return None;
        }
        if self
            .edges
            .iter()
            .any(|e| e.from_node == from_node && e.to_node == to_node)
        {
            return None;
        }
        let id = Self::next_id("e", self.edges.iter().map(|e| e.id.clone()));
        self.edges.push(CanvasEdge {
            id: id.clone(),
            from_node: from_node.to_string(),
            from_side,
            from_end: None,
            to_node: to_node.to_string(),
            to_side,
            to_end: Some(CanvasEnd::Arrow),
            color: None,
            label: label.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string),
        });
        Some(id)
    }

    pub fn remove_edge(&mut self, edge_id: &str) {
        self.edges.retain(|e| e.id != edge_id);
    }

    pub fn search_text(&self) -> String {
        let mut parts = Vec::new();
        if !self.title.trim().is_empty() {
            parts.push(self.title.trim().to_string());
        }
        for node in &self.nodes {
            if let Some(t) = &node.text {
                if !t.trim().is_empty() {
                    parts.push(t.trim().to_string());
                }
            }
            if let Some(lbl) = &node.label {
                if !lbl.trim().is_empty() {
                    parts.push(lbl.trim().to_string());
                }
            }
            if let Some(file) = &node.file {
                if !file.trim().is_empty() {
                    parts.push(vault_file_stem(file).to_string());
                }
            }
        }
        for edge in &self.edges {
            if let Some(lbl) = &edge.label {
                if !lbl.trim().is_empty() {
                    parts.push(lbl.trim().to_string());
                }
            }
        }
        parts.join("\n")
    }

    pub fn extract_note_links(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for node in &self.nodes {
            if node.kind == CanvasNodeKind::File {
                if let Some(file) = node.file.as_deref() {
                    let trimmed = file.trim();
                    if !trimmed.is_empty() {
                        let ctx = if self.title.is_empty() {
                            vault_file_stem(trimmed).to_string()
                        } else {
                            self.title.clone()
                        };
                        out.push((trimmed.to_string(), ctx));
                    }
                }
            }
            if let Some(text) = node.text.as_deref() {
                for wl in extract_wikilinks(text) {
                    out.push((wl.target, wl.context));
                }
            }
        }
        out
    }

    pub fn rewrite_note_links(
        &mut self,
        old_path: &str,
        new_path: &str,
        old_targets: &[&str],
        new_target: &str,
    ) -> usize {
        let mut total = 0usize;
        for node in &mut self.nodes {
            if node.kind == CanvasNodeKind::File {
                if let Some(file) = node.file.as_mut() {
                    let cur = file.trim();
                    let matches = cur.eq_ignore_ascii_case(old_path)
                        || old_targets.iter().any(|t| !t.trim().is_empty() && cur.eq_ignore_ascii_case(t.trim()));
                    if matches && !new_path.trim().is_empty() && cur != new_path.trim() {
                        *file = new_path.trim().to_string();
                        total += 1;
                    }
                }
            }
            if let Some(text) = node.text.as_mut() {
                let (updated, count) = rewrite_wikilinks(text, old_targets, new_target);
                if count > 0 {
                    *text = updated;
                    total += count;
                }
            }
        }
        total
    }

    /// Returns the smallest enclosing GroupNode containing the center of `node`.
    pub fn enclosing_group(&self, node: &CanvasNode) -> Option<&CanvasNode> {
        if node.kind == CanvasNodeKind::Group {
            return None;
        }
        let (cx, cy) = node.center();
        self.nodes
            .iter()
            .filter(|g| g.kind == CanvasNodeKind::Group && g.contains_point(cx, cy))
            .min_by(|a, b| {
                (a.width * a.height)
                    .partial_cmp(&(b.width * b.height))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Compiles the Canvas DAG + Group sections into a clean linear Markdown script/note.
    /// Cycles or disconnected islands fall back deterministically to spatial reading order.
    pub fn export_to_markdown(
        &self,
        read_file: impl Fn(&str) -> Option<(String, String)>,
    ) -> String {
        let mut groups: Vec<&CanvasNode> = self
            .nodes
            .iter()
            .filter(|n| n.kind == CanvasNodeKind::Group)
            .collect();
        groups.sort_by(|a, b| {
            let y_cmp = ((a.y / 140.0).floor() as i32).cmp(&((b.y / 140.0).floor() as i32));
            y_cmp
                .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.id.cmp(&b.id))
        });

        let group_rank: HashMap<&str, usize> = groups
            .iter()
            .enumerate()
            .map(|(i, g)| (g.id.as_str(), i))
            .collect();

        let content_nodes: Vec<&CanvasNode> = self
            .nodes
            .iter()
            .filter(|n| n.kind != CanvasNodeKind::Group)
            .collect();
        let node_by_id: HashMap<&str, &CanvasNode> =
            content_nodes.iter().map(|n| (n.id.as_str(), *n)).collect();

        let spatial_key = |n: &CanvasNode| -> (usize, i32, i32, i32, String) {
            let gr = self
                .enclosing_group(n)
                .and_then(|g| group_rank.get(g.id.as_str()).copied())
                .unwrap_or(usize::MAX);
            let y_band = (n.y / 140.0).floor() as i32;
            (gr, y_band, n.x.round() as i32, n.y.round() as i32, n.id.clone())
        };

        let mut indegree: HashMap<&str, usize> =
            content_nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            let from = edge.from_node.as_str();
            let to = edge.to_node.as_str();
            if from != to && node_by_id.contains_key(from) && node_by_id.contains_key(to) {
                adj.entry(from).or_default().push(to);
                if let Some(d) = indegree.get_mut(to) {
                    *d += 1;
                }
            }
        }

        let mut remaining: HashSet<&str> = content_nodes.iter().map(|n| n.id.as_str()).collect();
        let mut ordered: Vec<&CanvasNode> = Vec::with_capacity(content_nodes.len());

        while !remaining.is_empty() {
            let next_id = remaining
                .iter()
                .copied()
                .filter(|id| indegree.get(id).copied().unwrap_or(0) == 0)
                .min_by_key(|id| spatial_key(node_by_id[id]))
                .unwrap_or_else(|| {
                    // Break cycle deterministically using spatial reading order.
                    remaining
                        .iter()
                        .copied()
                        .min_by_key(|id| spatial_key(node_by_id[id]))
                        .expect("remaining is non-empty")
                });
            remaining.remove(next_id);
            ordered.push(node_by_id[next_id]);
            if let Some(neighbors) = adj.get(next_id) {
                for nxt in neighbors {
                    if let Some(d) = indegree.get_mut(nxt) {
                        *d = d.saturating_sub(1);
                    }
                }
            }
        }

        let mut incoming_label: HashMap<&str, &str> = HashMap::new();
        for edge in &self.edges {
            if let Some(lbl) = edge.label.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                incoming_label.entry(edge.to_node.as_str()).or_insert(lbl);
            }
        }

        let mut out = String::new();
        if !self.title.trim().is_empty() {
            out.push_str(&format!("# {}\n\n", self.title.trim()));
        }

        let mut active_group: Option<String> = None;
        for node in ordered {
            let enc = self.enclosing_group(node);
            let enc_id = enc.map(|g| g.id.clone());
            if enc_id != active_group {
                if let Some(g) = enc {
                    let g_title = g.display_title();
                    out.push_str(&format!("## {g_title}\n\n"));
                }
                active_group = enc_id;
            }

            if let Some(lbl) = incoming_label.get(node.id.as_str()) {
                out.push_str(&format!("> *{lbl}*\n\n"));
            }

            match node.kind {
                CanvasNodeKind::Text => {
                    let body = node.text.as_deref().unwrap_or("").trim();
                    if !body.is_empty() {
                        out.push_str(body);
                        out.push_str("\n\n");
                    }
                }
                CanvasNodeKind::File => {
                    let file_path = node.file.as_deref().unwrap_or("").trim();
                    if let Some((title, body)) = read_file(file_path) {
                        let clean_title = if title.trim().is_empty() {
                            vault_file_stem(file_path).to_string()
                        } else {
                            title.trim().to_string()
                        };
                        out.push_str(&format!("### [[{clean_title}]]\n"));
                        let excerpt = plain_excerpt(&body, 320);
                        if !excerpt.is_empty() {
                            out.push_str(&excerpt);
                            out.push_str("\n\n");
                        } else {
                            out.push('\n');
                        }
                    } else if !file_path.is_empty() {
                        let stem = vault_file_stem(file_path);
                        out.push_str(&format!("### [[{stem}]]\n\n"));
                    }
                }
                CanvasNodeKind::Link => {
                    if let Some(url) = node.url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                        out.push_str(&format!("- {url}\n\n"));
                    }
                }
                CanvasNodeKind::Group => {}
            }
        }

        while out.ends_with("\n\n\n") {
            out.pop();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_canvas_roundtrip_and_obsidian_compat() {
        let external = r#"{
  "nodes": [
    { "id": "n1", "type": "text", "x": 10, "y": 20, "width": 240, "height": 120, "color": "4", "text": "Hook" },
    { "id": "f1", "type": "file", "x": 300, "y": 20, "width": 220, "height": 100, "file": "architecture.md" }
  ],
  "edges": [
    { "id": "e1", "fromNode": "n1", "fromSide": "right", "toNode": "f1", "toSide": "left", "label": "details" }
  ]
}"#;
        let parsed = CanvasDoc::parse(external, "Video Storyboard").unwrap();
        assert_eq!(parsed.title, "Video Storyboard");
        assert_eq!(parsed.nodes.len(), 2);
        assert_eq!(parsed.edges.len(), 1);
        assert_eq!(parsed.edges[0].from_side, Some(CanvasSide::Right));
        assert_eq!(parsed.edges[0].label.as_deref(), Some("details"));

        let saved = parsed.serialize_pretty().unwrap();
        assert!(!saved.contains("\r\n"));
        let reparsed = CanvasDoc::parse(&saved, "Ignored").unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn extracts_and_rewrites_file_and_wikilinks() {
        let mut doc = CanvasDoc::new_storyboard("Demo", false);
        let n_file = doc.add_file_node(100.0, 200.0, "old-arch.md", Some("2"));
        let n_txt = doc.add_text_node(400.0, 200.0, "See [[Old Arch]] and ```[[Old Arch]]```", None);
        assert_ne!(n_file, n_txt);

        let links = doc.extract_note_links();
        assert!(links.iter().any(|(t, _)| t == "old-arch.md"));
        assert!(links.iter().any(|(t, _)| t == "Old Arch"));

        let changed = doc.rewrite_note_links(
            "old-arch.md",
            "architecture.md",
            &["Old Arch", "old-arch"],
            "Architecture",
        );
        assert_eq!(changed, 2);

        let file_node = doc.nodes.iter().find(|n| n.id == n_file).unwrap();
        assert_eq!(file_node.file.as_deref(), Some("architecture.md"));
        let txt_node = doc.nodes.iter().find(|n| n.id == n_txt).unwrap();
        assert_eq!(
            txt_node.text.as_deref(),
            Some("See [[Architecture]] and ```[[Old Arch]]```")
        );
    }

    #[test]
    fn exports_connected_chain_and_groups_to_linear_markdown() {
        let mut doc = CanvasDoc {
            title: "Выпуск #12".into(),
            pinned: false,
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        doc.add_group_node(0.0, 0.0, 400.0, 300.0, "01. Хук", Some("1"));
        doc.add_group_node(500.0, 0.0, 400.0, 300.0, "02. Финал", Some("4"));
        let c1 = doc.add_text_node(40.0, 60.0, "Захват внимания зрителя.", None);
        let c2 = doc.add_file_node(540.0, 60.0, "architecture.md", None);
        doc.add_edge(&c1, Some(CanvasSide::Right), &c2, Some(CanvasSide::Left), Some("Переход"));

        let md = doc.export_to_markdown(|path| {
            if path == "architecture.md" {
                Some(("Architecture".into(), "# Core\nOpenGL + SQLite FTS5.".into()))
            } else {
                None
            }
        });
        assert!(md.starts_with("# Выпуск #12\n\n## 01. Хук"));
        assert!(md.contains("Захват внимания зрителя."));
        assert!(md.contains("## 02. Финал"));
        assert!(md.contains("> *Переход*"));
        assert!(md.contains("### [[Architecture]]"));
        assert!(md.contains("Core OpenGL + SQLite FTS5."));

        let idx1 = md.find("Захват внимания").unwrap();
        let idx2 = md.find("### [[Architecture]]").unwrap();
        assert!(idx1 < idx2);
    }

    #[test]
    fn builds_curated_canvas_presets() {
        let blank = CanvasDoc::new_preset("Blank", "blank", false);
        assert!(blank.nodes.is_empty());

        let mind = CanvasDoc::new_preset("Brainstorm", "mindmap", true);
        assert_eq!(mind.nodes.len(), 5);
        assert_eq!(mind.edges.len(), 4);

        let arch = CanvasDoc::new_preset("System", "architecture", false);
        assert_eq!(arch.nodes.len(), 6);
        assert_eq!(arch.edges.len(), 2);

        let story = CanvasDoc::new_preset("Video", "storyboard", true);
        assert_eq!(story.nodes.len(), 6);
        assert_eq!(story.edges.len(), 2);
    }
}
