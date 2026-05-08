import type {
  BlockExportFormat,
  BlockExportResult,
  BlockPlugin,
  BlockTypstExportResult,
  EditorPlugin,
} from "./types";

type PluginModule = {
  default?: EditorPlugin;
  editorPlugin?: EditorPlugin;
  blockPlugin?: BlockPlugin;
  blockPlugins?: BlockPlugin[];
};

function cloneBlockPlugin(plugin: BlockPlugin): BlockPlugin {
  return { ...plugin };
}

function normalizeEditorPlugin(module: PluginModule): EditorPlugin | null {
  if (module.default?.blocks?.length) {
    return module.default;
  }
  if (module.editorPlugin?.blocks?.length) {
    return module.editorPlugin;
  }
  if (module.blockPlugins?.length) {
    return {
      id: "legacy.blocks",
      blocks: module.blockPlugins,
    };
  }
  if (module.blockPlugin) {
    return {
      id: "legacy.block",
      blocks: [module.blockPlugin],
    };
  }
  return null;
}

function loadEditorPlugins(): EditorPlugin[] {
  const modules = import.meta.glob<PluginModule>("./*.ts", { eager: true });
  const entries = Object.entries(modules)
    .filter(([path]) => path !== "./registry.ts" && path !== "./types.ts")
    .sort(([left], [right]) => left.localeCompare(right));

  const loaded: EditorPlugin[] = [];
  for (const [, moduleDef] of entries) {
    const normalized = normalizeEditorPlugin(moduleDef);
    if (!normalized) {
      continue;
    }
    loaded.push(normalized);
  }
  return loaded;
}

const editorPlugins = loadEditorPlugins();
const blockPlugins = editorPlugins.flatMap((plugin) => plugin.blocks.map(cloneBlockPlugin));
const pluginMap = new Map<string, BlockPlugin>(blockPlugins.map((plugin) => [plugin.type, plugin]));

export function listEditorPlugins(): EditorPlugin[] {
  return editorPlugins.map((plugin) => ({
    ...plugin,
    blocks: plugin.blocks.map(cloneBlockPlugin),
  }));
}

export function listBlockPlugins(): BlockPlugin[] {
  return blockPlugins.map(cloneBlockPlugin);
}

export function getBlockPlugin(type: string): BlockPlugin | null {
  return pluginMap.get(type) ?? null;
}

export function createDefaultBlockForType(type: string) {
  const plugin = getBlockPlugin(type);
  const block = plugin?.createDefaultBlock();
  return block ? { ...block } : null;
}

export async function exportBlockWithPlugin(
  context: {
    type: string;
    block: Parameters<NonNullable<BlockPlugin["exportBlock"]>>[0]["block"];
    note: Parameters<NonNullable<BlockPlugin["exportBlock"]>>[0]["note"];
    index: number;
  },
  format: BlockExportFormat,
): Promise<BlockExportResult> {
  const plugin = getBlockPlugin(context.type);
  if (!plugin?.exportBlock) {
    throw new Error(`Block type '${context.type}' does not support export.`);
  }

  return plugin.exportBlock(
    {
      block: context.block,
      note: context.note,
      index: context.index,
    },
    format,
  );
}

export async function exportTypstWithPlugin(
  context: {
    type: string;
    block: Parameters<NonNullable<BlockPlugin["exportTypst"]>>[0]["block"];
    note: Parameters<NonNullable<BlockPlugin["exportTypst"]>>[0]["note"];
    index: number;
  },
): Promise<BlockTypstExportResult> {
  const plugin = getBlockPlugin(context.type);
  if (!plugin?.exportTypst) {
    throw new Error(`Block type '${context.type}' does not support Typst export.`);
  }

  return plugin.exportTypst({
    block: context.block,
    note: context.note,
    index: context.index,
  });
}
