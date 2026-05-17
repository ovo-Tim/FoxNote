import { onBeforeUnmount, ref, toValue, watch, type MaybeRefOrGetter, type Ref } from "vue";
import { LocalLinter, WorkerLinter, type Lint, type LintOptions, type Suggestion } from "harper.js";
import { binaryInlined } from "harper.js/binaryInlined";

export type SpellcheckMode = "plaintext" | "typst";

export interface SpellcheckSuggestionItem {
  id: string;
  label: string;
  suggestion: Suggestion;
}

export interface SpellcheckIssue {
  id: string;
  start: number;
  end: number;
  message: string;
  kind: string;
  problemText: string;
  suggestions: SpellcheckSuggestionItem[];
  lint: Lint;
}

interface SpellcheckLinterLike<TLint = Lint, TSuggestion = Suggestion> {
  setup(): Promise<void>;
  lint(text: string, options?: LintOptions): Promise<TLint[]>;
  applySuggestion(text: string, lint: TLint, suggestion: TSuggestion): Promise<string>;
}

interface UseSpellcheckFieldOptions {
  source: MaybeRefOrGetter<string>;
  mode: MaybeRefOrGetter<SpellcheckMode>;
  enabled?: MaybeRefOrGetter<boolean>;
  debounceMs?: number;
  onApplied: (nextText: string) => void;
}

const DEFAULT_DEBOUNCE_MS = 260;
const LINTER_TIMEOUT_MS = 2200;
export const TYPST_FORMULA_REGEX_MASK = String.raw`(?s)\$(?:\\.|[^$\\])*\$`;

let sharedLinterPromise: Promise<SpellcheckLinterLike> | null = null;
let preferredLinterKind: "worker" | "local" = "worker";

function hasSpellcheckableText(text: string): boolean {
  return /[A-Za-z][A-Za-z]/.test(text);
}

function createIssueId(lint: Lint, index: number): string {
  const span = lint.span();
  return `${span.start}:${span.end}:${lint.message()}:${index}`;
}

function createSuggestionId(issueId: string, index: number): string {
  return `${issueId}:suggestion:${index}`;
}

function getSuggestionLabel(suggestion: Suggestion): string {
  const text = suggestion.get_replacement_text();
  if (!text) {
    return suggestion.kind() === 1 ? "Remove" : "Apply";
  }
  return text;
}

function maybeLogContractionLint(lint: Lint, issueId: string, sourceText?: string) {
  const span = lint.span();
  const problemText = lint.get_problem_text();
  const message = lint.message();
  const sourceSlice = sourceText?.slice(Math.max(0, span.start - 8), Math.min(sourceText.length, span.end + 8)) ?? "";
  const exactSlice = sourceText?.slice(span.start, span.end) ?? "";

  if (
    !/[A-Za-z]['’][A-Za-z]+/.test(problemText) &&
    !/[A-Za-z]['’][A-Za-z]+/.test(exactSlice) &&
    !/[A-Za-z]['’][A-Za-z]+/.test(sourceSlice) &&
    !/\b(?:['’]s|['’]re|['’]ve|['’]ll|['’]d|['’]m|n't)\b/i.test(problemText) &&
    !/\b(?:['’]s|['’]re|['’]ve|['’]ll|['’]d|['’]m|n't)\b/i.test(exactSlice) &&
    !/\b(?:['’]s|['’]re|['’]ve|['’]ll|['’]d|['’]m|n't)\b/i.test(sourceSlice)
  ) {
    return;
  }

  console.warn("[Spellcheck][ContractionLint]", {
    issueId,
    start: span.start,
    end: span.end,
    problemText,
    exactSlice,
    sourceSlice,
    message,
    kind: lint.lint_kind_pretty(),
    suggestions: lint.suggestions().slice(0, 3).map((suggestion) => suggestion.get_replacement_text()),
  });
}

function shouldSuppressFalsePositiveLint(lint: Lint, sourceText: string): boolean {
  const span = lint.span();
  const message = lint.message();
  const kind = lint.lint_kind_pretty();
  const exactSlice = sourceText.slice(span.start, span.end);
  const sourceSlice = sourceText.slice(Math.max(0, span.start - 12), Math.min(sourceText.length, span.end + 12));

  if (
    kind === "Word Choice" &&
    /words would go better together/i.test(message) &&
    /^[A-Za-z]['’]?[A-Za-z]?\s+[A-Za-z]['’]?[A-Za-z]?$/i.test(exactSlice) &&
    /[?!.]\s*$/.test(sourceText.slice(Math.max(0, span.start - 3), span.start))
  ) {
    return true;
  }

  if (
    kind === "Word Choice" &&
    /words would go better together/i.test(message) &&
    /\b[A-Za-z]+['’]s\b/i.test(sourceSlice) &&
    /\b[A-Za-z]+\s+[A-Za-z]+\b/.test(exactSlice)
  ) {
    return true;
  }

  return false;
}

function mapLintToIssue(lint: Lint, index: number): SpellcheckIssue {
  const span = lint.span();
  const issueId = createIssueId(lint, index);
  return {
    id: issueId,
    start: span.start,
    end: span.end,
    message: lint.message(),
    kind: lint.lint_kind_pretty(),
    problemText: lint.get_problem_text(),
    suggestions: lint.suggestions().slice(0, 3).map((suggestion, suggestionIndex) => ({
      id: createSuggestionId(issueId, suggestionIndex),
      label: getSuggestionLabel(suggestion),
      suggestion,
    })),
    lint,
  };
}

function withTimeout<T>(promise: Promise<T>, label: string, timeoutMs = LINTER_TIMEOUT_MS): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`${label} timed out after ${timeoutMs}ms`));
    }, timeoutMs);

    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
}

async function createWorkerSpellcheckLinter(): Promise<SpellcheckLinterLike> {
  const linter = new WorkerLinter({
    binary: binaryInlined,
  });
  await withTimeout(linter.setup(), "Harper worker setup");
  return linter;
}

async function createLocalSpellcheckLinter(): Promise<SpellcheckLinterLike> {
  const linter = new LocalLinter({
    binary: binaryInlined,
  });
  await withTimeout(linter.setup(), "Harper local setup", Math.max(LINTER_TIMEOUT_MS, 4000));
  return linter;
}

async function createBestAvailableLinter(): Promise<SpellcheckLinterLike> {
  if (preferredLinterKind === "local") {
    return createLocalSpellcheckLinter();
  }

  try {
    return await createWorkerSpellcheckLinter();
  } catch {
    preferredLinterKind = "local";
    return createLocalSpellcheckLinter();
  }
}

async function getSharedLinter(): Promise<SpellcheckLinterLike> {
  if (!sharedLinterPromise) {
    sharedLinterPromise = createBestAvailableLinter().catch((error) => {
      sharedLinterPromise = null;
      throw error;
    });
  }

  return sharedLinterPromise;
}

async function lintWithFallback(text: string, options: LintOptions): Promise<Lint[]> {
  try {
    const linter = await getSharedLinter();
    return await withTimeout(linter.lint(text, options), "Harper lint");
  } catch (error) {
    if (preferredLinterKind === "local") {
      throw error;
    }

    preferredLinterKind = "local";
    sharedLinterPromise = null;
    const localLinter = await getSharedLinter();
    return withTimeout(localLinter.lint(text, options), "Harper local lint", Math.max(LINTER_TIMEOUT_MS, 4000));
  }
}

export function createSpellcheckLintOptions(mode: SpellcheckMode): LintOptions {
  if (mode === "typst") {
    return {
      language: "typst",
      regex_mask: TYPST_FORMULA_REGEX_MASK,
    };
  }

  return {
    language: "plaintext",
  };
}

export async function applySpellcheckSuggestion<TLint, TSuggestion>(
  linter: SpellcheckLinterLike<TLint, TSuggestion>,
  text: string,
  lint: TLint,
  suggestion: TSuggestion,
): Promise<string> {
  return linter.applySuggestion(text, lint, suggestion);
}

export function useSpellcheckField(options: UseSpellcheckFieldOptions): {
  issues: Ref<SpellcheckIssue[]>;
  loading: Ref<boolean>;
  error: Ref<string>;
  applySuggestion: (issueId: string, suggestionId: string) => Promise<void>;
  refresh: () => Promise<void>;
} {
  const issues = ref<SpellcheckIssue[]>([]);
  const loading = ref(false);
  const error = ref("");
  const debounceMs = options.debounceMs ?? DEFAULT_DEBOUNCE_MS;
  let lintTimer: ReturnType<typeof setTimeout> | null = null;
  let lintRunId = 0;

  function clearLintTimer() {
    if (!lintTimer) {
      return;
    }
    clearTimeout(lintTimer);
    lintTimer = null;
  }

  async function refresh(nextText?: string): Promise<void> {
    const enabled = options.enabled === undefined ? true : toValue(options.enabled);
    const text = nextText ?? toValue(options.source);

    if (!enabled || !text.trim() || !hasSpellcheckableText(text)) {
      clearLintTimer();
      issues.value = [];
      error.value = "";
      loading.value = false;
      return;
    }

    const runId = ++lintRunId;
    loading.value = true;

    try {
      const lints = await lintWithFallback(text, createSpellcheckLintOptions(toValue(options.mode)));
      if (runId !== lintRunId) {
        return;
      }

      const filteredLints = lints.filter((lint) => !shouldSuppressFalsePositiveLint(lint, text));

      if (/[A-Za-z]['’][A-Za-z]+/.test(text)) {
        filteredLints.forEach((lint, index) => {
          maybeLogContractionLint(lint, createIssueId(lint, index), text);
        });
      }

      issues.value = filteredLints.map((lint, index) => mapLintToIssue(lint, index));
      error.value = "";
    } catch (reason) {
      if (runId !== lintRunId) {
        return;
      }

      issues.value = [];
      error.value = reason instanceof Error ? reason.message : String(reason);
    } finally {
      if (runId === lintRunId) {
        loading.value = false;
      }
    }
  }

  function scheduleRefresh() {
    clearLintTimer();
    lintTimer = setTimeout(() => {
      lintTimer = null;
      void refresh();
    }, debounceMs);
  }

  async function applySuggestionById(issueId: string, suggestionId: string): Promise<void> {
    const issue = issues.value.find((entry) => entry.id === issueId);
    if (!issue) {
      return;
    }

    const suggestion = issue.suggestions.find((entry) => entry.id === suggestionId);
    if (!suggestion) {
      return;
    }

    const sourceText = toValue(options.source);
    const linter = await getSharedLinter();
    const nextText = await applySpellcheckSuggestion(linter, sourceText, issue.lint, suggestion.suggestion);
    options.onApplied(nextText);
    await refresh(nextText);
  }

  watch(
    () => [toValue(options.source), toValue(options.mode), options.enabled === undefined ? true : toValue(options.enabled)],
    () => {
      scheduleRefresh();
    },
    { immediate: true },
  );

  onBeforeUnmount(() => {
    clearLintTimer();
  });

  return {
    issues,
    loading,
    error,
    applySuggestion: applySuggestionById,
    refresh,
  };
}
