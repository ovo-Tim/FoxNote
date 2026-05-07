use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("failed to run git: {0}")]
    GitIo(#[from] std::io::Error),
    #[error("git command failed: {0}")]
    GitCommand(String),
    #[error("sync repository is not initialized")]
    RepoNotInitialized,
    #[error("sync remote is not configured")]
    RemoteNotConfigured,
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
            self.run_git(&["init"])?;
        }

        if self.current_branch().is_err() {
            self.run_git(&["checkout", "-b", "main"])?;
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

        if self.remote_url()?.is_some() {
            self.run_git(&["remote", "set-url", "origin", trimmed])?;
        } else {
            self.run_git(&["remote", "add", "origin", trimmed])?;
        }

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

        self.run_git(&["add", "-A"])?;
        if self.has_staged_changes()? {
            let message = format!("FoxNote sync {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
            self.run_git(&["commit", "-m", &message])?;
        }

        if !self.has_any_commit()? {
            self.run_git(&["commit", "--allow-empty", "-m", "FoxNote sync bootstrap"])?;
        }

        self.run_git(&["fetch", "origin"])?;
        let branch = self.current_branch().unwrap_or_else(|_| "main".to_string());
        let remote_branch = format!("origin/{branch}");

        if self.remote_branch_exists(&remote_branch)? {
            let merge_result = self.run_git(&["merge", "--no-edit", &remote_branch]);
            if merge_result.is_err() && !self.collect_conflicts()?.is_empty() {
                return Ok(self.with_message(
                    SyncPhase::Conflict,
                    "Sync conflict detected. Choose local A or remote B for each note.",
                )?);
            }
            merge_result?;
        }

        self.run_git(&["push", "origin", &branch])?;

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

        let branch = self.current_branch().unwrap_or_else(|_| "main".to_string());
        let pull_result = self.run_git(&["pull", "--no-edit", "origin", &branch]);
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
        let pull_status = self.pull_only()?;
        if matches!(
            pull_status.phase,
            SyncPhase::Conflict | SyncPhase::NeedsSetup
        ) {
            return Ok(pull_status);
        }

        let branch = self.current_branch().unwrap_or_else(|_| "main".to_string());
        self.run_git(&["push", "origin", &branch])?;

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

        self.run_git(&["add", "-A"])?;
        if !self.has_staged_changes()? {
            return self.with_message(SyncPhase::Idle, "No changes to commit");
        }

        let normalized = message.trim();
        let commit_message = if normalized.is_empty() {
            "Update note"
        } else {
            normalized
        };

        self.run_git(&["commit", "-m", commit_message])?;

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

        let flag = if use_local { "--ours" } else { "--theirs" };
        for file in files {
            self.run_git(&["checkout", flag, "--", &file])?;
            self.run_git(&["add", "--", &file])?;
        }

        self.with_message(SyncPhase::Conflict, "Conflict choice applied")
    }

    pub fn finalize_conflicts(&self) -> Result<SyncStatus, SyncError> {
        let remaining = self.collect_conflicts()?;
        if !remaining.is_empty() {
            return Ok(self.with_message(SyncPhase::Conflict, "Unresolved conflicts remain")?);
        }

        if self.merge_head_exists() {
            self.run_git(&["commit", "--no-edit"])?;
        }

        let branch = self.current_branch().unwrap_or_else(|_| "main".to_string());
        self.run_git(&["push", "origin", &branch])?;

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
        self.run_git(&["symbolic-ref", "--short", "HEAD"])
            .map(|text| text.trim().to_string())
    }

    fn remote_url(&self) -> Result<Option<String>, SyncError> {
        match self.run_git(&["remote", "get-url", "origin"]) {
            Ok(url) => {
                let trimmed = url.trim();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed.to_string()))
                }
            }
            Err(SyncError::GitCommand(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn has_staged_changes(&self) -> Result<bool, SyncError> {
        let output = self.run_git_output(&["diff", "--cached", "--quiet"])?;
        Ok(!output.status.success())
    }

    fn has_any_commit(&self) -> Result<bool, SyncError> {
        let output = self.run_git_output(&["rev-parse", "--verify", "HEAD"])?;
        Ok(output.status.success())
    }

    fn remote_branch_exists(&self, remote_branch: &str) -> Result<bool, SyncError> {
        let output = self.run_git_output(&["rev-parse", "--verify", remote_branch])?;
        Ok(output.status.success())
    }

    fn collect_conflicts(&self) -> Result<Vec<SyncConflict>, SyncError> {
        let output = self.run_git(&["diff", "--name-only", "--diff-filter=U"])?;
        let files = output
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
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

    fn run_git(&self, args: &[&str]) -> Result<String, SyncError> {
        let output = self.run_git_output(args)?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }

        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let command = format!("git {}", args.join(" "));
        Err(SyncError::GitCommand(if error.is_empty() {
            command
        } else {
            format!("{command}: {error}")
        }))
    }

    fn run_git_output(&self, args: &[&str]) -> Result<std::process::Output, SyncError> {
        fs::create_dir_all(&self.repo_root)?;
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .args(args)
            .env("GIT_AUTHOR_NAME", "FoxNote")
            .env("GIT_AUTHOR_EMAIL", "foxnote@local")
            .env("GIT_COMMITTER_NAME", "FoxNote")
            .env("GIT_COMMITTER_EMAIL", "foxnote@local")
            .output()?;
        Ok(output)
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
            self.run_git(&["remote", "add", "origin", &remote_url])?;
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
