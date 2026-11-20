use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncOp {
    Push(String),
    Pull { path: String, remote_sha: String },
    DeleteLocal(String),
    DeleteRemote(String),
    Conflict { path: String, remote_sha: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncPlan {
    pub ops: Vec<SyncOp>,
    pub unchanged: HashMap<String, String>,
}

/// Deterministic 3-way sync planner across (base_sha, local_sha, remote_sha).
pub fn plan_sync(
    base: &HashMap<String, String>,
    local: &HashMap<String, String>,
    remote: &HashMap<String, String>,
) -> SyncPlan {
    let mut all_paths = BTreeSet::new();
    for k in base.keys().chain(local.keys()).chain(remote.keys()) {
        all_paths.insert(k.clone());
    }

    let mut plan = SyncPlan::default();

    for path in all_paths {
        let b = base.get(&path).map(String::as_str);
        let l = local.get(&path).map(String::as_str);
        let r = remote.get(&path).map(String::as_str);

        match (l, r) {
            (Some(l_sha), Some(r_sha)) => {
                if l_sha == r_sha {
                    plan.unchanged.insert(path, l_sha.to_string());
                } else if b == Some(r_sha) {
                    plan.ops.push(SyncOp::Push(path));
                } else if b == Some(l_sha) {
                    plan.ops.push(SyncOp::Pull {
                        path,
                        remote_sha: r_sha.to_string(),
                    });
                } else {
                    plan.ops.push(SyncOp::Conflict {
                        path,
                        remote_sha: r_sha.to_string(),
                    });
                }
            }
            (Some(l_sha), None) => {
                if b == Some(l_sha) {
                    plan.ops.push(SyncOp::DeleteLocal(path));
                } else {
                    // Either new locally (b == None) or modified locally while remote deleted (b != l)
                    plan.ops.push(SyncOp::Push(path));
                }
            }
            (None, Some(r_sha)) => {
                if b == Some(r_sha) {
                    plan.ops.push(SyncOp::DeleteRemote(path));
                } else {
                    // Either new remotely (b == None) or modified remotely while local deleted (b != r)
                    plan.ops.push(SyncOp::Pull {
                        path,
                        remote_sha: r_sha.to_string(),
                    });
                }
            }
            (None, None) => {
                // Deleted on both sides: omit from unchanged/new_base.
            }
        }
    }

    plan
}

/// Counts local changes (new, modified, deleted files) relative to the last synced `base` hashes.
pub fn count_local_changes(
    base: &HashMap<String, String>,
    local: &HashMap<String, String>,
) -> usize {
    let mut changes = 0usize;
    for (path, l_sha) in local {
        if base.get(path) != Some(l_sha) {
            changes += 1;
        }
    }
    for path in base.keys() {
        if !local.contains_key(path) {
            changes += 1;
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn covers_all_three_way_merge_cases() {
        let base = map(&[
            ("same.md", "h1"),
            ("push.md", "h1"),
            ("pull.md", "h1"),
            ("conflict.md", "h1"),
            ("del_local.md", "h1"),
            ("edit_vs_del_remote.md", "h1"),
            ("del_remote.md", "h1"),
            ("del_local_vs_edit_remote.md", "h1"),
            ("both_deleted.md", "h1"),
        ]);
        let local = map(&[
            ("same.md", "h1"),
            ("push.md", "h2"),
            ("pull.md", "h1"),
            ("conflict.md", "h2"),
            ("del_local.md", "h1"),
            ("new_local.md", "h9"),
            ("edit_vs_del_remote.md", "h2"),
            ("new_both_same.md", "hs"),
            ("new_both_diff.md", "hl"),
        ]);
        let remote = map(&[
            ("same.md", "h1"),
            ("push.md", "h1"),
            ("pull.md", "h3"),
            ("conflict.md", "h3"),
            ("del_remote.md", "h1"),
            ("del_local_vs_edit_remote.md", "h3"),
            ("new_remote.md", "h8"),
            ("new_both_same.md", "hs"),
            ("new_both_diff.md", "hr"),
        ]);

        let plan = plan_sync(&base, &local, &remote);

        assert_eq!(
            plan.unchanged,
            map(&[("same.md", "h1"), ("new_both_same.md", "hs")])
        );
        assert_eq!(
            plan.ops,
            vec![
                SyncOp::Conflict {
                    path: "conflict.md".into(),
                    remote_sha: "h3".into(),
                },
                SyncOp::DeleteLocal("del_local.md".into()),
                SyncOp::Pull {
                    path: "del_local_vs_edit_remote.md".into(),
                    remote_sha: "h3".into(),
                },
                SyncOp::DeleteRemote("del_remote.md".into()),
                SyncOp::Push("edit_vs_del_remote.md".into()),
                SyncOp::Conflict {
                    path: "new_both_diff.md".into(),
                    remote_sha: "hr".into(),
                },
                SyncOp::Push("new_local.md".into()),
                SyncOp::Pull {
                    path: "new_remote.md".into(),
                    remote_sha: "h8".into(),
                },
                SyncOp::Pull {
                    path: "pull.md".into(),
                    remote_sha: "h3".into(),
                },
                SyncOp::Push("push.md".into()),
            ]
        );
    }

    #[test]
    fn handles_empty_remote_rename_folder_delete_and_local_count() {
        // Initial push into empty remote
        let base_empty = map(&[]);
        let local_init = map(&[("welcome.md", "a1"), ("daily/2026-11-19.md", "b2")]);
        let remote_empty = map(&[]);
        assert_eq!(count_local_changes(&base_empty, &local_init), 2);
        let init_plan = plan_sync(&base_empty, &local_init, &remote_empty);
        assert_eq!(
            init_plan.ops,
            vec![
                SyncOp::Push("daily/2026-11-19.md".into()),
                SyncOp::Push("welcome.md".into()),
            ]
        );

        // Local rename + remote folder delete
        let base = map(&[
            ("old.md", "h1"),
            ("notes/a.md", "ha"),
            ("notes/b.md", "hb"),
            ("keep.md", "hk"),
        ]);
        let local = map(&[
            ("new.md", "h1"),
            ("notes/a.md", "ha"),
            ("notes/b.md", "hb"),
            ("keep.md", "hk2"),
        ]);
        let remote = map(&[("old.md", "h1"), ("keep.md", "hk")]);
        assert_eq!(count_local_changes(&base, &local), 3); // new.md added, old.md deleted, keep.md edited
        let plan = plan_sync(&base, &local, &remote);
        assert_eq!(
            plan.ops,
            vec![
                SyncOp::Push("keep.md".into()),
                SyncOp::Push("new.md".into()),
                SyncOp::DeleteLocal("notes/a.md".into()),
                SyncOp::DeleteLocal("notes/b.md".into()),
                SyncOp::DeleteRemote("old.md".into()),
            ]
        );
    }
}
