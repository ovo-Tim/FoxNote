import type { BlockPlugin } from "./types";
import { imageBlockPlugin } from "./image";
import { typstBlockPlugin } from "./typst";

const builtInPlugins: BlockPlugin[] = [typstBlockPlugin, imageBlockPlugin];

const pluginMap = new Map<string, BlockPlugin>(
  builtInPlugins.map((plugin) => [plugin.type, plugin]),
);

export function listBlockPlugins(): BlockPlugin[] {
  return builtInPlugins.map((plugin) => ({ ...plugin }));
}

export function getBlockPlugin(type: string): BlockPlugin | null {
  return pluginMap.get(type) ?? null;
}

export function createDefaultBlockForType(type: string) {
  const plugin = getBlockPlugin(type);
  return plugin?.createDefaultBlock() ?? null;
}
