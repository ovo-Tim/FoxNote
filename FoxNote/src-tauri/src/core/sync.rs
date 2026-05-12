use chrono::Local;
use git2::{
    build::CheckoutBuilder, Config, Cred, CredentialType, ErrorCode, FetchOptions, IndexAddOption,
    MergeOptions, PushOptions, RemoteCallbacks, Repository, Signature, Status, StatusOptions,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("git operation failed: {0}")]
    Git(#[from] git2::Error),
    #[error("sync repository is not initialized")]
    RepoNotInitialized,
    #[error("sync remote is not configured")]
    RemoteNotConfigured,
    #[error("sync merge conflict detected")]
    MergeConflict,
    #[error("no conflicting files found for '{0}'")]
    ConflictNotFound(String),
    #[error("sync state lock poisoned")]
    StatePoisoned,
    #[error("failed to parse sync config: {0}")]
    ConfigParse(#[from] toml::de::Error),
    #[error("failed to serialize sync config: {0}")]
    ConfigSerialize(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    NeedsSetup,
    Idle,
    Syncing,
    Conflict,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflict {
    pub note_id: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub phase: SyncPhase,
    pub repo_initialized: bool,
    pub remote_url: Option<String>,
    pub branch: Option<String>,
    pub auto_sync_enabled: bool,
    pub auto_sync_interval_sec: u64,
    pub last_sync_at: Option<String>,
    pub message: Option<String>,
    pub conflicts: Vec<SyncConflict>,
}

#[derive(Debug, Clone)]
struct SyncRuntime {
    auto_sync_enabled: bool,
    auto_sync_interval_sec: u64,
    last_sync_at: Option<String>,
    configured_remote_url: Option<String>,
}

impl Default for SyncRuntime {
    fn default() -> Self {
        Self {
            auto_sync_enabled: false,
            auto_sync_interval_sec: 180,
            last_sync_at: None,
            configured_remote_url: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct SyncConfigFile {
    version: u32,
    sync: SyncConfigSection,
}

#[derive(Debug, Deserialize, Serialize)]
struct SyncConfigSection {
    auto_sync_enabled: bool,
    auto_sync_interval_sec: u64,
    remote_url: Option<String>,
}

#[derive(Debug)]
pub struct SyncService {
    repo_root: PathBuf,
    runtime: Mutex<SyncRuntime>,
}

impl SyncService {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        let repo_root = repo_root.into();
        let runtime = Self::load_runtime_from_config(&repo_root).unwrap_or_default();

        let service = Self {
            repo_root,
            runtime: Mutex::new(runtime),
        };

        let _ = service.apply_remote_hint();

        service
    }

    pub fn config_path_string(&self) -> String {
        self.config_path().to_string_lossy().into_owned()
    }

    pub fn status(&self) -> Result<SyncStatus, SyncError> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| SyncError::StatePoisoned)?
            .clone();

        let repo_initialized = self.is_repo_initialized();
        let remote_url = if repo_initialized {
            self.remote_url().ok().flatten()
        } else {
            runtime.configured_remote_url.clone()
        };
        let branch = if repo_initialized {
            self.current_branch().ok()
        } else {
            None
        };
        let conflicts = if repo_initialized {
            self.collect_conflicts().unwrap_or_default()
        } else {
            Vec::new()
        };

        let phase = if !repo_initialized || remote_url.is_none() {
            SyncPhase::NeedsSetup
        } else if !conflicts.is_empty() {
            SyncPhase::Conflict
        } else {
            SyncPhase::Idle
        };

        Ok(SyncStatus {
            phase,
            repo_initialized,
            remote_url,
            branch,
            auto_sync_enabled: runtime.auto_sync_enabled,
            auto_sync_interval_sec: runtime.auto_sync_interval_sec,
            last_sync_at: runtime.last_sync_at,
            message: None,
            conflicts,
        })
    }

    pub fn set_auto_sync(&self, enabled: bool, interval_sec: u64) -> Result<SyncStatus, SyncError> {
        let interval = interval_sec.clamp(30, 3600);
        {
            let mut runtime = self.runtime.lock().map_err(|_| SyncError::StatePoisoned)?;
            runtime.auto_sync_enabled = enabled;
            runtime.auto_sync_interval_sec = interval;
        }
        self.persist_runtime_to_config()?;
        self.status()
    }

    pub fn init_repo(&self) -> Result<SyncStatus, SyncError> {
        if !self.is_repo_initialized() {
            fs::create_dir_all(&self.repo_root)?;
            let _ = Repository::init(&self.repo_root)?;
        }

        if self.current_branch().is_err() {
            let repo = self.repository()?;
            repo.set_head("refs/heads/main")?;
        }

        if self.remote_url()?.is_some() {
            return self.status();
        }

        self.apply_remote_hint()?;

        self.status()
    }

    pub fn set_remote(&self, remote_url: &str) -> Result<SyncStatus, SyncError> {
        let trimmed = remote_url.trim();
        if trimmed.is_empty() {
            return Err(SyncError::RemoteNotConfigured);
        }

        {
            let mut runtime = self.runtime.lock().map_err(|_| SyncError::StatePoisoned)?;
            runtime.configured_remote_url = Some(trimmed.to_string());
        }
        self.persist_runtime_to_config()?;

        if !self.is_repo_initialized() {
            return self.with_message(
                SyncPhase::NeedsSetup,
                "Remote saved. Initialize repository to apply it.",
            );
        }

        let repo = self.repository()?;
        self.set_origin_remote(&repo, trimmed)?;

        self.status()
    }

    pub fn sync_now(&self) -> Result<SyncStatus, SyncError> {
        if !self.is_repo_initialized() {
            return Ok(self.with_message(SyncPhase::NeedsSetup, "Initialize repository first")?);
        }

        if self.remote_url()?.is_none() {
            return Ok(self.with_message(SyncPhase::NeedsSetup, "Configure remote URL first")?);
        }

        if !self.collect_conflicts()?.is_empty() {
            return Ok(self.with_message(
                SyncPhase::Conflict,
                "Resolve sync conflicts before next sync",
            )?);
        }

        let repo = self.repository()?;
        self.stage_all(&repo)?;
        if self.has_staged_changes()? {
            let message = format!("FoxNote sync {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
            self.commit_all(&repo, &message)?;
        }

        if !self.has_any_commit()? {
            self.commit_all(&repo, "FoxNote sync bootstrap")?;
        }

        self.fetch_origin(&repo)?;
        let branch = self.current_branch().unwrap_or_else(|_| "main".to_string());
        let remote_branch = format!("origin/{branch}");

        if self.remote_branch_exists(&remote_branch)? {
            let merge_result = self.merge_remote_branch(&repo, &branch);
            if merge_result.is_err() && !self.collect_conflicts()?.is_empty() {
                return Ok(self.with_message(
                    SyncPhase::Conflict,
                    "Sync conflict detected. Choose local A or remote B for each note.",
                )?);
            }
            merge_result?;
        }

        self.push_branch(&repo, &branch)?;

        let mut runtime = self.runtime.lock().map_err(|_| SyncError::StatePoisoned)?;
        runtime.last_sync_at = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        drop(runtime);

        self.with_message(SyncPhase::Idle, "Sync completed")
    }

    pub fn pull_only(&self) -> Result<SyncStatus, SyncError> {
        if !self.is_repo_initialized() {
            return Ok(self.with_message(SyncPhase::NeedsSetup, "Initialize repository first")?);
        }

        if self.remote_url()?.is_none() {
            return Ok(self.with_message(SyncPhase::NeedsSetup, "Configure remote URL first")?);
        }

        if !self.collect_conflicts()?.is_empty() {
            return Ok(
                self.with_message(SyncPhase::Conflict, "Resolve sync conflicts before pull")?
            );
        }

        let repo = self.repository()?;
        let branch = self.current_branch().unwrap_or_else(|_| "main".to_string());
        self.fetch_origin(&repo)?;
        let pull_result = self.merge_remote_branch(&repo, &branch);
        if pull_result.is_err() && !self.collect_conflicts()?.is_empty() {
            return Ok(self.with_message(
                SyncPhase::Conflict,
                "Pull conflict detected. Resolve before continuing.",
            )?);
        }
        pull_result?;

        let mut runtime = self.runtime.lock().map_err(|_| SyncError::StatePoisoned)?;
        runtime.last_sync_at = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        drop(runtime);

        self.with_message(SyncPhase::Idle, "Pull completed")
    }

    pub fn pull_then_push(&self) -> Result<SyncStatus, SyncError> {
        if !self.is_repo_initialized() {
            return Ok(self.with_message(SyncPhase::NeedsSetup, "Initialize repository first")?);
        }

        if !self.collect_conflicts()?.is_empty() {
            return Ok(
                self.with_message(SyncPhase::Conflict, "Resolve sync conflicts before upload")?
            );
        }

        let repo = self.repository()?;
        self.stage_all(&repo)?;
        if self.has_staged_changes()? {
            let message = format!(
                "FoxNote upload {}",
                Local::now().format("%Y-%m-%d %H:%M:%S")
            );
            self.commit_all(&repo, &message)?;
        }

        let pull_status = self.pull_only()?;
        if matches!(
            pull_status.phase,
            SyncPhase::Conflict | SyncPhase::NeedsSetup
        ) {
            return Ok(pull_status);
        }

        let branch = self.current_branch().unwrap_or_else(|_| "main".to_string());
        self.push_branch(&repo, &branch)?;

        let mut runtime = self.runtime.lock().map_err(|_| SyncError::StatePoisoned)?;
        runtime.last_sync_at = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        drop(runtime);

        self.with_message(SyncPhase::Idle, "Pull and push completed")
    }

    pub fn commit_only(&self, message: &str) -> Result<SyncStatus, SyncError> {
        if !self.is_repo_initialized() {
            return Ok(self.with_message(SyncPhase::NeedsSetup, "Initialize repository first")?);
        }

        if !self.collect_conflicts()?.is_empty() {
            return Ok(
                self.with_message(SyncPhase::Conflict, "Resolve sync conflicts before commit")?
            );
        }

        let repo = self.repository()?;
        self.stage_all(&repo)?;
        if !self.has_staged_changes()? {
            return self.with_message(SyncPhase::Idle, "No changes to commit");
        }

        let normalized = message.trim();
        let commit_message = if normalized.is_empty() {
            "Update note"
        } else {
            normalized
        };

        self.commit_all(&repo, commit_message)?;

        let mut runtime = self.runtime.lock().map_err(|_| SyncError::StatePoisoned)?;
        runtime.last_sync_at = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        drop(runtime);

        self.with_message(SyncPhase::Idle, "Commit completed")
    }

    pub fn note_has_changes(&self, note_id: &str) -> Result<bool, SyncError> {
        if !self.is_repo_initialized() {
            return Ok(false);
        }

        let note_path = normalize_note_path(note_id);
        if note_path.is_empty() {
            return Ok(false);
        }

        let repo = self.repository()?;
        let mut options = StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .pathspec(&note_path);

        let statuses = repo.statuses(Some(&mut options))?;
        Ok(!statuses.is_empty())
    }

    pub fn commit_note_only(&self, note_id: &str, message: &str) -> Result<SyncStatus, SyncError> {
        if !self.is_repo_initialized() {
            return Ok(self.with_message(SyncPhase::NeedsSetup, "Initialize repository first")?);
        }

        if !self.collect_conflicts()?.is_empty() {
            return Ok(
                self.with_message(SyncPhase::Conflict, "Resolve sync conflicts before commit")?
            );
        }

        let note_path = normalize_note_path(note_id);
        if note_path.is_empty() {
            return self.with_message(SyncPhase::Idle, "No changes to commit");
        }

        let repo = self.repository()?;
        self.stage_path(&repo, &note_path)?;
        if !self.note_has_changes(&note_path)? {
            return self.with_message(SyncPhase::Idle, "No changes to commit");
        }

        let normalized = message.trim();
        let commit_message = if normalized.is_empty() {
            "Update note"
        } else {
            normalized
        };

        self.commit_all(&repo, commit_message)?;

        let mut runtime = self.runtime.lock().map_err(|_| SyncError::StatePoisoned)?;
        runtime.last_sync_at = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        drop(runtime);

        self.with_message(SyncPhase::Idle, "Commit completed")
    }

    pub fn resolve_conflict(
        &self,
        note_id: &str,
        use_local: bool,
    ) -> Result<SyncStatus, SyncError> {
        let normalized = note_id.trim().trim_matches('/');
        let files = self.collect_conflict_files_for_note(normalized)?;
        if files.is_empty() {
            return Err(SyncError::ConflictNotFound(note_id.to_string()));
        }

        let repo = self.repository()?;
        for file in files {
            let mut checkout = CheckoutBuilder::new();
            checkout.force().path(&file);
            if use_local {
                checkout.use_ours(true);
            } else {
                checkout.use_theirs(true);
            }
            repo.checkout_index(None, Some(&mut checkout))?;

            let mut index = repo.index()?;
            let rel = Path::new(&file);
            let _ = index.conflict_remove(rel);
            if self.repo_root.join(rel).exists() {
                index.add_path(rel)?;
            } else {
                let _ = index.remove_path(rel);
            }
            index.write()?;
        }

        self.with_message(SyncPhase::Conflict, "Conflict choice applied")
    }

    pub fn finalize_conflicts(&self) -> Result<SyncStatus, SyncError> {
        let remaining = self.collect_conflicts()?;
        if !remaining.is_empty() {
            return Ok(self.with_message(SyncPhase::Conflict, "Unresolved conflicts remain")?);
        }

        let repo = self.repository()?;
        if self.merge_head_exists() {
            self.finalize_merge_state_commit(&repo)?;
        }

        let branch = self.current_branch().unwrap_or_else(|_| "main".to_string());
        self.push_branch(&repo, &branch)?;

        let mut runtime = self.runtime.lock().map_err(|_| SyncError::StatePoisoned)?;
        runtime.last_sync_at = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        drop(runtime);

        self.with_message(SyncPhase::Idle, "Conflicts resolved and synced")
    }

    fn with_message(&self, phase: SyncPhase, message: &str) -> Result<SyncStatus, SyncError> {
        let mut status = self.status()?;
        status.phase = phase;
        status.message = Some(message.to_string());
        Ok(status)
    }

    fn is_repo_initialized(&self) -> bool {
        self.repo_root.join(".git").is_dir()
    }

    fn merge_head_exists(&self) -> bool {
        self.repo_root.join(".git").join("MERGE_HEAD").exists()
    }

    fn current_branch(&self) -> Result<String, SyncError> {
        let repo = self.repository()?;
        let head = repo.head()?;
        let branch = head
            .shorthand()
            .ok_or_else(|| git2::Error::from_str("unable to resolve current branch"))?;
        Ok(branch.to_string())
    }

    fn remote_url(&self) -> Result<Option<String>, SyncError> {
        let repo = self.repository()?;
        let result = match repo.find_remote("origin") {
            Ok(remote) => Ok(remote
                .url()
                .map(str::to_string)
                .filter(|url| !url.trim().is_empty())),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        };
        result
    }

    fn has_staged_changes(&self) -> Result<bool, SyncError> {
        let repo = self.repository()?;
        let mut options = StatusOptions::new();
        options
            .include_untracked(false)
            .recurse_untracked_dirs(false)
            .renames_head_to_index(true);

        let staged_mask = Status::INDEX_NEW
            | Status::INDEX_MODIFIED
            | Status::INDEX_DELETED
            | Status::INDEX_RENAMED
            | Status::INDEX_TYPECHANGE;

        let statuses = repo.statuses(Some(&mut options))?;
        Ok(statuses
            .iter()
            .any(|entry| entry.status().intersects(staged_mask)))
    }

    fn has_any_commit(&self) -> Result<bool, SyncError> {
        let repo = self.repository()?;
        let has_commit = repo.head().ok().and_then(|head| head.target()).is_some();
        Ok(has_commit)
    }

    fn remote_branch_exists(&self, remote_branch: &str) -> Result<bool, SyncError> {
        let repo = self.repository()?;
        let full_ref = format!("refs/remotes/{remote_branch}");
        let result = match repo.find_reference(&full_ref) {
            Ok(_) => Ok(true),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        };
        result
    }

    fn collect_conflicts(&self) -> Result<Vec<SyncConflict>, SyncError> {
        if !self.is_repo_initialized() {
            return Ok(Vec::new());
        }

        let repo = self.repository()?;
        let index = repo.index()?;
        let conflicts_iter = index.conflicts()?;

        let files = conflicts_iter
            .filter_map(|entry| {
                entry.ok().and_then(|conflict| {
                    conflict
                        .our
                        .as_ref()
                        .or(conflict.their.as_ref())
                        .or(conflict.ancestor.as_ref())
                        .map(|index_entry| String::from_utf8_lossy(&index_entry.path).to_string())
                })
            })
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<String>>();

        let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for file in files {
            let note_id = note_id_from_conflict_path(&file);
            grouped.entry(note_id).or_default().push(file);
        }

        let conflicts = grouped
            .into_iter()
            .map(|(note_id, files)| SyncConflict { note_id, files })
            .collect();

        Ok(conflicts)
    }

    fn collect_conflict_files_for_note(&self, note_id: &str) -> Result<Vec<String>, SyncError> {
        let files = self
            .collect_conflicts()?
            .into_iter()
            .filter(|entry| entry.note_id == note_id)
            .flat_map(|entry| entry.files)
            .collect();
        Ok(files)
    }

    fn repository(&self) -> Result<Repository, SyncError> {
        fs::create_dir_all(&self.repo_root)?;
        match Repository::open(&self.repo_root) {
            Ok(repo) => Ok(repo),
            Err(error) if error.code() == ErrorCode::NotFound => Err(SyncError::RepoNotInitialized),
            Err(error) => Err(error.into()),
        }
    }

    fn signature(&self) -> Result<Signature<'static>, SyncError> {
        Ok(Signature::now("FoxNote", "foxnote@local")?)
    }

    fn remote_callbacks(&self) -> RemoteCallbacks<'static> {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|url, username_from_url, allowed_types| {
            if allowed_types.contains(CredentialType::USER_PASS_PLAINTEXT)
                || allowed_types.contains(CredentialType::DEFAULT)
            {
                if let Ok(config) = Config::open_default() {
                    if let Ok(cred) = Cred::credential_helper(&config, url, username_from_url) {
                        return Ok(cred);
                    }
                }
            }

            if allowed_types.contains(CredentialType::SSH_KEY) {
                if let Some(username) = username_from_url {
                    if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                        return Ok(cred);
                    }
                }
                if let Ok(cred) = Cred::ssh_key_from_agent("git") {
                    return Ok(cred);
                }
            }

            if allowed_types.contains(CredentialType::USERNAME) {
                if let Some(username) = username_from_url {
                    return Cred::username(username);
                }
                return Cred::username("git");
            }

            Cred::default()
        });
        callbacks
    }

    fn stage_all(&self, repo: &Repository) -> Result<(), SyncError> {
        let mut index = repo.index()?;
        index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
        index.write()?;
        Ok(())
    }

    fn stage_path(&self, repo: &Repository, path: &str) -> Result<(), SyncError> {
        let mut index = repo.index()?;
        index.add_all([path], IndexAddOption::DEFAULT, None)?;
        index.write()?;
        Ok(())
    }

    fn commit_all(&self, repo: &Repository, message: &str) -> Result<(), SyncError> {
        let signature = self.signature()?;
        let mut index = repo.index()?;
        let tree_oid = index.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;

        if let Ok(head) = repo.head() {
            if let Ok(parent) = head.peel_to_commit() {
                repo.commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    message,
                    &tree,
                    &[&parent],
                )?;
                return Ok(());
            }
        }

        repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])?;
        Ok(())
    }

    fn set_origin_remote(&self, repo: &Repository, url: &str) -> Result<(), SyncError> {
        match repo.find_remote("origin") {
            Ok(_) => {
                repo.remote_set_url("origin", url)?;
            }
            Err(error) if error.code() == ErrorCode::NotFound => {
                let _ = repo.remote("origin", url)?;
            }
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    fn fetch_origin(&self, repo: &Repository) -> Result<(), SyncError> {
        let mut remote = repo.find_remote("origin")?;
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(self.remote_callbacks());
        remote.fetch(&[] as &[&str], Some(&mut fetch_options), None)?;
        Ok(())
    }

    fn merge_remote_branch(&self, repo: &Repository, branch: &str) -> Result<(), SyncError> {
        let remote_ref = format!("refs/remotes/origin/{branch}");
        let reference = match repo.find_reference(&remote_ref) {
            Ok(reference) => reference,
            Err(error) if error.code() == ErrorCode::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };

        let annotated = repo.reference_to_annotated_commit(&reference)?;
        let (analysis, _) = repo.merge_analysis(&[&annotated])?;

        if analysis.is_up_to_date() {
            return Ok(());
        }

        if analysis.is_fast_forward() {
            let target = annotated.id();
            self.fast_forward(repo, branch, target)?;
            return Ok(());
        }

        if analysis.is_normal() {
            let mut merge_options = MergeOptions::new();
            let mut checkout = CheckoutBuilder::new();
            checkout
                .allow_conflicts(true)
                .conflict_style_merge(true)
                .safe();

            repo.merge(&[&annotated], Some(&mut merge_options), Some(&mut checkout))?;

            let index = repo.index()?;
            if index.has_conflicts() {
                return Err(SyncError::MergeConflict);
            }

            self.finalize_merge_state_commit(repo)?;
            return Ok(());
        }

        Ok(())
    }

    fn fast_forward(
        &self,
        repo: &Repository,
        branch: &str,
        target: git2::Oid,
    ) -> Result<(), SyncError> {
        let local_ref = format!("refs/heads/{branch}");
        match repo.find_reference(&local_ref) {
            Ok(mut reference) => {
                reference.set_target(target, "Fast-forward")?;
            }
            Err(error) if error.code() == ErrorCode::NotFound => {
                let _ = repo.reference(&local_ref, target, true, "Create branch")?;
            }
            Err(error) => return Err(error.into()),
        }

        repo.set_head(&local_ref)?;
        let mut checkout = CheckoutBuilder::new();
        checkout.force();
        repo.checkout_head(Some(&mut checkout))?;
        Ok(())
    }

    fn finalize_merge_state_commit(&self, repo: &Repository) -> Result<(), SyncError> {
        if !self.merge_head_exists() {
            return Ok(());
        }

        let merge_head_path = repo.path().join("MERGE_HEAD");
        let merge_head_text = fs::read_to_string(merge_head_path)?;
        let merge_head_oid = merge_head_text
            .lines()
            .find(|line| !line.trim().is_empty())
            .ok_or_else(|| git2::Error::from_str("MERGE_HEAD is empty"))?
            .trim()
            .parse::<git2::Oid>()?;

        let head_commit = repo.head()?.peel_to_commit()?;
        let merge_commit = repo.find_commit(merge_head_oid)?;

        let mut index = repo.index()?;
        let tree_oid = index.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;

        let signature = self.signature()?;
        let message = repo
            .message()
            .unwrap_or_else(|_| "Merge commit".to_string());

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &message,
            &tree,
            &[&head_commit, &merge_commit],
        )?;

        repo.checkout_head(Some(CheckoutBuilder::new().safe()))?;
        repo.cleanup_state()?;
        Ok(())
    }

    fn push_branch(&self, repo: &Repository, branch: &str) -> Result<(), SyncError> {
        let mut remote = repo.find_remote("origin")?;
        let mut push_options = PushOptions::new();
        push_options.remote_callbacks(self.remote_callbacks());
        let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
        remote.push(&[refspec.as_str()], Some(&mut push_options))?;
        Ok(())
    }

    fn config_path(&self) -> PathBuf {
        self.repo_root.join("foxnote.toml")
    }

    fn persist_runtime_to_config(&self) -> Result<(), SyncError> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| SyncError::StatePoisoned)?
            .clone();

        fs::create_dir_all(&self.repo_root)?;
        let config = SyncConfigFile {
            version: 1,
            sync: SyncConfigSection {
                auto_sync_enabled: runtime.auto_sync_enabled,
                auto_sync_interval_sec: runtime.auto_sync_interval_sec,
                remote_url: runtime.configured_remote_url,
            },
        };
        let text = toml::to_string_pretty(&config)?;
        fs::write(self.config_path(), text)?;
        Ok(())
    }

    fn load_runtime_from_config(repo_root: &Path) -> Result<SyncRuntime, SyncError> {
        let path = repo_root.join("foxnote.toml");
        if !path.exists() {
            return Ok(SyncRuntime::default());
        }

        let text = fs::read_to_string(path)?;
        let config: SyncConfigFile = toml::from_str(&text)?;
        let interval = config.sync.auto_sync_interval_sec.clamp(30, 3600);

        Ok(SyncRuntime {
            auto_sync_enabled: config.sync.auto_sync_enabled,
            auto_sync_interval_sec: interval,
            last_sync_at: None,
            configured_remote_url: config.sync.remote_url,
        })
    }

    fn apply_remote_hint(&self) -> Result<(), SyncError> {
        if !self.is_repo_initialized() {
            return Ok(());
        }

        let has_remote = self.remote_url()?.is_some();
        if has_remote {
            return Ok(());
        }

        let configured_remote = self
            .runtime
            .lock()
            .map_err(|_| SyncError::StatePoisoned)?
            .configured_remote_url
            .clone();

        if let Some(remote_url) = configured_remote {
            let repo = self.repository()?;
            self.set_origin_remote(&repo, &remote_url)?;
        }

        Ok(())
    }
}

fn note_id_from_conflict_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let path_ref = Path::new(&normalized);

    if let Some(parent) = path_ref.parent() {
        let parent_string = parent
            .iter()
            .map(|segment| segment.to_string_lossy().into_owned())
            .collect::<Vec<String>>()
            .join("/");
        if !parent_string.is_empty() {
            return parent_string;
        }
    }

    normalized
}

fn normalize_note_path(note_id: &str) -> String {
    note_id
        .trim()
        .replace('\\', "/")
        .trim_matches('/')
        .to_string()
}
