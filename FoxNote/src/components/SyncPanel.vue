<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { SyncConflict, SyncStatus } from "../types/note";

const props = defineProps<{
  status: SyncStatus | null;
  busy?: boolean;
  configPath?: string;
}>();

const emit = defineEmits<{
  init: [];
  setRemote: [remoteUrl: string];
  syncNow: [];
  setAutoSync: [enabled: boolean, intervalSec: number];
  resolveConflict: [noteId: string, useLocal: boolean];
  finalizeConflicts: [];
  refresh: [];
}>();

const remoteUrlInput = ref("");
const intervalInput = ref(180);

watch(
  () => props.status,
  (status) => {
    if (!status) {
      return;
    }
    remoteUrlInput.value = status.remoteUrl ?? "";
    intervalInput.value = status.autoSyncIntervalSec || 180;
  },
  { immediate: true },
);

const phaseLabel = computed(() => {
  const phase = props.status?.phase;
  if (!phase) {
    return "loading";
  }

  switch (phase) {
    case "needs_setup":
      return "needs setup";
    case "idle":
      return "idle";
    case "syncing":
      return "syncing";
    case "conflict":
      return "conflict";
    default:
      return "error";
  }
});

const phaseClass = computed(() => {
  const phase = props.status?.phase;
  if (!phase) {
    return "";
  }
  return `is-${phase.replace("_", "-")}`;
});

const hasConflicts = computed(() => {
  return (props.status?.conflicts.length ?? 0) > 0;
});

function submitRemote() {
  emit("setRemote", remoteUrlInput.value.trim());
}

function toggleAutoSync(value: boolean | null) {
  emit("setAutoSync", Boolean(value), intervalInput.value);
}

function updateInterval() {
  const safe = Math.max(30, Math.min(3600, Number(intervalInput.value) || 180));
  intervalInput.value = safe;
  emit("setAutoSync", Boolean(props.status?.autoSyncEnabled), safe);
}

function chooseLocal(conflict: SyncConflict) {
  emit("resolveConflict", conflict.noteId, true);
}

function chooseRemote(conflict: SyncConflict) {
  emit("resolveConflict", conflict.noteId, false);
}
</script>

<template>
  <section class="sync-page">
    <header class="sync-page-header">
      <div>
        <h2 class="sync-page-title">Sync Settings</h2>
        <p class="sync-page-subtitle">Manage repository setup, remote URL, auto sync, and conflict resolution.</p>
      </div>
      <v-btn size="small" variant="text" prepend-icon="mdi-refresh" :disabled="busy" @click="emit('refresh')">Refresh</v-btn>
    </header>

    <p class="sync-config-path">Config file: <code>{{ configPath || "foxnote.toml" }}</code></p>

    <section class="sync-card sync-grid">
      <div>
        <p class="sync-card-section-title">Status</p>
        <p class="sync-status-row">
          <span class="status-badge" :class="phaseClass">{{ phaseLabel }}</span>
          <span class="sync-branch">Branch: {{ status?.branch || "main" }}</span>
        </p>

        <div class="sync-meta-grid">
          <span>Repository: {{ status?.repoInitialized ? "initialized" : "not initialized" }}</span>
          <span>Last sync: {{ status?.lastSyncAt || "never" }}</span>
        </div>

        <p v-if="status?.message" class="message-line sync-message">{{ status.message }}</p>
      </div>

      <div>
        <p class="sync-card-section-title">Repository Setup</p>
        <v-btn
          v-if="!status?.repoInitialized"
          block
          variant="tonal"
          prepend-icon="mdi-source-repository-plus"
          :loading="busy"
          @click="emit('init')"
        >
          Initialize Repository
        </v-btn>

        <template v-else>
          <div class="sync-row">
            <v-text-field
              v-model="remoteUrlInput"
              density="comfortable"
              variant="outlined"
              hide-details
              label="Remote URL"
              placeholder="https://... or git@..."
              @keydown.enter.prevent="submitRemote"
            />
            <v-btn variant="tonal" :loading="busy" @click="submitRemote">Save</v-btn>
          </div>

          <div class="sync-action-row">
            <v-btn color="primary" prepend-icon="mdi-sync" :loading="busy" :disabled="!status?.remoteUrl" @click="emit('syncNow')">
              Sync Now
            </v-btn>
          </div>
        </template>
      </div>
    </section>

    <section class="sync-card">
      <p class="sync-card-section-title">Auto Sync</p>
      <div class="sync-row compact">
        <v-switch
          :model-value="status?.autoSyncEnabled ?? false"
          density="compact"
          hide-details
          color="primary"
          label="Enable auto sync"
          :disabled="busy"
          @update:model-value="toggleAutoSync"
        />
        <v-text-field
          v-model.number="intervalInput"
          class="interval-field"
          type="number"
          min="30"
          max="3600"
          density="compact"
          variant="outlined"
          hide-details
          suffix="sec"
          :disabled="busy || !(status?.autoSyncEnabled ?? false)"
          @change="updateInterval"
        />
      </div>
    </section>

    <section v-if="hasConflicts" class="sync-card conflict-zone">
      <h3 class="conflict-title">Conflicts</h3>
      <div v-for="conflict in status?.conflicts" :key="conflict.noteId" class="conflict-item">
        <p class="conflict-note-id">{{ conflict.noteId }}</p>
        <p class="conflict-files">{{ conflict.files.join(", ") }}</p>
        <div class="choice-row">
          <v-btn size="small" variant="tonal" color="primary" :loading="busy" @click="chooseLocal(conflict)">
            Keep Local A
          </v-btn>
          <v-btn size="small" variant="tonal" color="warning" :loading="busy" @click="chooseRemote(conflict)">
            Keep Remote B
          </v-btn>
        </div>
      </div>

      <v-btn block color="success" variant="flat" prepend-icon="mdi-check-bold" :loading="busy" @click="emit('finalizeConflicts')">
        Finalize and Push
      </v-btn>
    </section>
  </section>
</template>

<style scoped>
.sync-page {
  max-width: 920px;
  margin: 0 auto;
  padding: 0.95rem 1rem 1.2rem;
  display: grid;
  gap: 0.62rem;
}

.sync-page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 0.8rem;
}

.sync-page-title {
  margin: 0;
  font-size: clamp(1.28rem, 2vw, 1.62rem);
  color: var(--fox-text-strong);
}

.sync-page-subtitle {
  margin: 0.24rem 0 0;
  font-size: 0.9rem;
  color: var(--fox-text-muted);
}

.sync-config-path {
  margin: 0;
  font-size: 0.8rem;
  color: var(--fox-text-muted);
}

.sync-card {
  border: 1px solid var(--fox-border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--fox-surface) 95%, #0f1728 5%);
  padding: 0.75rem;
}

.sync-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.65rem;
}

.sync-card-section-title {
  margin: 0 0 0.42rem;
  font-size: 0.88rem;
  color: var(--fox-text-muted);
}

.sync-status-row {
  margin: 0;
  display: flex;
  align-items: center;
  gap: 0.45rem;
  flex-wrap: wrap;
}

.status-badge {
  border: 1px solid var(--fox-border);
  border-radius: 999px;
  padding: 0.12rem 0.5rem;
  font-size: 0.72rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--fox-text-strong);
  background: color-mix(in srgb, var(--fox-chip) 75%, transparent 25%);
}

.status-badge.is-conflict {
  border-color: rgba(255, 169, 92, 0.45);
  background: rgba(255, 169, 92, 0.16);
  color: #ffd3a3;
}

.status-badge.is-needs-setup {
  border-color: rgba(137, 171, 255, 0.45);
  background: rgba(137, 171, 255, 0.16);
  color: #c7d8ff;
}

.status-badge.is-error {
  border-color: rgba(255, 102, 102, 0.42);
  background: rgba(255, 102, 102, 0.16);
  color: #ffc2c2;
}

.sync-branch {
  color: var(--fox-text-muted);
  font-size: 0.82rem;
}

.sync-meta-grid {
  margin-top: 0.45rem;
  display: grid;
  gap: 0.26rem;
  font-size: 0.82rem;
  color: var(--fox-text-muted);
}

.sync-message {
  margin: 0.56rem 0 0;
}

.message-line,
.conflict-files,
.conflict-note-id {
  margin: 0;
  font-size: 0.82rem;
  color: var(--fox-text-muted);
}

.sync-row {
  display: flex;
  gap: 0.45rem;
}

.sync-row > :first-child {
  flex: 1;
}

.sync-row.compact {
  align-items: center;
  margin-top: 0.1rem;
}

.sync-action-row {
  display: flex;
  justify-content: flex-end;
  margin-top: 0.48rem;
}

.interval-field {
  width: 130px;
}

.conflict-zone {
  display: grid;
  gap: 0.45rem;
}

.conflict-title {
  margin: 0;
  font-size: 0.9rem;
  color: var(--fox-text-strong);
}

.conflict-item {
  border: 1px solid var(--fox-border);
  border-radius: 9px;
  padding: 0.5rem;
  display: grid;
  gap: 0.28rem;
}

.choice-row {
  display: flex;
  gap: 0.35rem;
}

@media (max-width: 980px) {
  .sync-page {
    padding: 0.66rem;
  }

  .sync-grid {
    grid-template-columns: 1fr;
  }

  .sync-row {
    flex-direction: column;
  }

  .interval-field {
    width: 100%;
  }
}
</style>
