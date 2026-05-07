<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import type { InstallPluginInput, PluginEntry, PluginSourceKind } from "../types/note";

const props = defineProps<{
  plugins: PluginEntry[];
  busy?: boolean;
  configPath?: string;
}>();

const emit = defineEmits<{
  install: [input: InstallPluginInput];
  toggle: [pluginId: string, enabled: boolean];
  remove: [pluginId: string];
  refresh: [];
}>();

const installForm = reactive<InstallPluginInput>({
  id: "",
  name: "",
  sourceKind: "local",
  source: "",
});

const query = ref("");

const sourceOptions: { title: string; value: PluginSourceKind }[] = [
  { title: "Local", value: "local" },
  { title: "Remote", value: "remote" },
];

const filteredPlugins = computed(() => {
  const normalized = query.value.trim().toLowerCase();
  if (!normalized) {
    return props.plugins;
  }

  return props.plugins.filter((plugin) => {
    const text = `${plugin.name} ${plugin.id} ${plugin.source}`.toLowerCase();
    return text.includes(normalized);
  });
});

function handleInstall() {
  emit("install", {
    id: installForm.id.trim(),
    name: installForm.name.trim(),
    sourceKind: installForm.sourceKind,
    source: installForm.source.trim(),
  });

  installForm.id = "";
  installForm.name = "";
  installForm.source = "";
}

function isLocal(kind: PluginSourceKind): boolean {
  return kind === "local";
}
</script>

<template>
  <section class="plugins-page">
    <header class="plugins-page-header">
      <div>
        <h2 class="plugins-page-title">Plugins</h2>
        <p class="plugins-page-subtitle">Install trusted local or remote plugins to extend block types.</p>
      </div>
      <v-btn size="small" variant="text" prepend-icon="mdi-refresh" :disabled="busy" @click="emit('refresh')">Refresh</v-btn>
    </header>

    <p class="plugins-config-path">Config file: <code>{{ configPath || "plugins.toml" }}</code></p>

    <section class="plugins-card">
      <p class="plugins-card-title">Install Plugin</p>
      <div class="install-grid">
        <v-text-field v-model="installForm.id" density="comfortable" variant="outlined" hide-details label="Plugin ID"
          placeholder="my-plugin" />
        <v-text-field v-model="installForm.name" density="comfortable" variant="outlined" hide-details label="Display name"
          placeholder="My Plugin" />
        <v-select v-model="installForm.sourceKind" :items="sourceOptions" density="comfortable" variant="outlined"
          hide-details label="Source kind" item-title="title" item-value="value" />
      </div>
      <div class="install-row">
        <v-text-field v-model="installForm.source" density="comfortable" variant="outlined" hide-details label="Source"
          :placeholder="installForm.sourceKind === 'local' ? './plugins/my-plugin' : 'https://example.com/plugin.git'"
          @keydown.enter.prevent="handleInstall" />
        <v-btn color="primary" :disabled="busy" :loading="busy" prepend-icon="mdi-package-variant-closed-plus"
          @click="handleInstall">
          Install
        </v-btn>
      </div>
    </section>

    <section class="plugins-card">
      <div class="plugins-list-head">
        <p class="plugins-card-title">Installed Plugins</p>
        <v-text-field v-model="query" density="compact" variant="solo-filled" hide-details rounded="pill"
          prepend-inner-icon="mdi-magnify" placeholder="Filter plugins" class="plugins-filter" />
      </div>

      <div class="plugins-list">
        <article v-for="plugin in filteredPlugins" :key="plugin.id" class="plugin-item">
          <div class="plugin-item-main">
            <div class="plugin-name-row">
              <p class="plugin-name">{{ plugin.name }}</p>
              <span class="plugin-id">{{ plugin.id }}</span>
            </div>
            <p class="plugin-source">
              <v-icon :icon="isLocal(plugin.sourceKind) ? 'mdi-folder-outline' : 'mdi-cloud-outline'" size="14" />
              <span>{{ plugin.source }}</span>
            </p>
            <p class="plugin-installed">Installed at {{ plugin.installedAt }}</p>
          </div>

          <div class="plugin-item-actions">
            <v-switch :model-value="plugin.enabled" hide-details density="compact" inset :disabled="busy"
              @update:model-value="(value) => emit('toggle', plugin.id, Boolean(value))" />
            <v-btn size="small" variant="text" color="error" prepend-icon="mdi-delete-outline" :disabled="busy"
              @click="emit('remove', plugin.id)">
              Remove
            </v-btn>
          </div>
        </article>

        <p v-if="filteredPlugins.length === 0" class="plugins-empty">No plugins found.</p>
      </div>
    </section>
  </section>
</template>

<style scoped>
.plugins-page {
  max-width: 960px;
  margin: 0 auto;
  padding: 0.95rem 1rem 1.2rem;
  display: grid;
  gap: 0.62rem;
}

.plugins-page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 0.8rem;
}

.plugins-page-title {
  margin: 0;
  font-size: clamp(1.28rem, 2vw, 1.62rem);
  color: var(--fox-text-strong);
}

.plugins-page-subtitle {
  margin: 0.24rem 0 0;
  font-size: 0.9rem;
  color: var(--fox-text-muted);
}

.plugins-config-path {
  margin: 0;
  font-size: 0.8rem;
  color: var(--fox-text-muted);
}

.plugins-card {
  border: 1px solid var(--fox-border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--fox-surface) 95%, #0f1728 5%);
  padding: 0.75rem;
}

.plugins-card-title {
  margin: 0 0 0.42rem;
  font-size: 0.88rem;
  color: var(--fox-text-muted);
}

.install-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.5rem;
}

.install-row {
  margin-top: 0.5rem;
  display: flex;
  gap: 0.5rem;
}

.install-row :first-child {
  flex: 1;
}

.plugins-list-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.7rem;
}

.plugins-filter {
  max-width: 260px;
}

.plugins-list {
  margin-top: 0.3rem;
  display: grid;
  gap: 0.42rem;
}

.plugin-item {
  border: 1px solid var(--fox-border);
  border-radius: 10px;
  background: color-mix(in srgb, var(--fox-surface) 90%, black 10%);
  padding: 0.55rem;
  display: flex;
  justify-content: space-between;
  gap: 0.65rem;
}

.plugin-item-main {
  min-width: 0;
}

.plugin-name-row {
  display: flex;
  align-items: baseline;
  gap: 0.45rem;
  flex-wrap: wrap;
}

.plugin-name {
  margin: 0;
  color: var(--fox-text-strong);
  font-weight: 600;
}

.plugin-id {
  font-family: "Iosevka", "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 0.82rem;
  color: var(--fox-text-muted);
}

.plugin-source {
  margin: 0.2rem 0 0;
  color: var(--fox-text-body);
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.86rem;
  overflow: hidden;
  text-overflow: ellipsis;
}

.plugin-installed {
  margin: 0.16rem 0 0;
  font-size: 0.78rem;
  color: var(--fox-text-muted);
}

.plugin-item-actions {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.plugins-empty {
  margin: 0.2rem 0 0;
  color: var(--fox-text-muted);
  font-size: 0.84rem;
}

@media (max-width: 980px) {
  .plugins-page {
    padding: 0.66rem;
  }

  .install-grid {
    grid-template-columns: 1fr;
  }

  .install-row {
    flex-direction: column;
  }

  .plugins-list-head {
    flex-direction: column;
    align-items: stretch;
  }

  .plugins-filter {
    max-width: none;
  }

  .plugin-item {
    flex-direction: column;
  }

  .plugin-item-actions {
    justify-content: space-between;
  }
}
</style>
