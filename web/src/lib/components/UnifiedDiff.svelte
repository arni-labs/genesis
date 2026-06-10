<script lang="ts">
  type DiffLine = {
    kind: 'add' | 'del' | 'ctx' | 'meta';
    text: string;
  };

  type DiffFile = {
    path: string;
    lines: DiffLine[];
    additions: number;
    deletions: number;
  };

  type UnifiedDiffProps = {
    patch: string;
    maxFiles?: number;
    maxLinesPerFile?: number;
    prioritizeAppSpecs?: boolean;
  };

  let {
    patch,
    maxFiles = 4,
    maxLinesPerFile = 18,
    prioritizeAppSpecs = true
  }: UnifiedDiffProps = $props();

  const parsedFiles = $derived(parsePatch(patch));
  const visibleFiles = $derived(
    (prioritizeAppSpecs ? rankedDiffFiles(parsedFiles) : parsedFiles).slice(0, maxFiles)
  );
  const hiddenFileCount = $derived(Math.max(0, parsedFiles.length - visibleFiles.length));

  function parsePatch(value: string): DiffFile[] {
    const parsed: DiffFile[] = [];
    let current: DiffFile | null = null;
    for (const rawLine of value.split('\n')) {
      if (rawLine.startsWith('diff --git ')) {
        if (current) parsed.push(current);
        current = {
          path: filePathFromDiffHeader(rawLine),
          lines: [{ kind: 'meta', text: rawLine }],
          additions: 0,
          deletions: 0
        };
        continue;
      }
      if (!current) {
        if (!rawLine.trim()) continue;
        current = { path: 'patch', lines: [], additions: 0, deletions: 0 };
      }
      const kind =
        rawLine.startsWith('+') && !rawLine.startsWith('+++')
          ? 'add'
          : rawLine.startsWith('-') && !rawLine.startsWith('---')
            ? 'del'
            : rawLine.startsWith('@@') || rawLine.startsWith('index ') || rawLine.startsWith('---') || rawLine.startsWith('+++')
              ? 'meta'
              : 'ctx';
      if (kind === 'add') current.additions += 1;
      if (kind === 'del') current.deletions += 1;
      current.lines.push({ kind, text: rawLine });
    }
    if (current) parsed.push(current);
    return parsed.filter((file) => file.lines.length > 0);
  }

  function rankedDiffFiles(files: DiffFile[]): DiffFile[] {
    return [...files].sort((left, right) => {
      const rank = diffFileRank(left.path) - diffFileRank(right.path);
      return rank || left.path.localeCompare(right.path);
    });
  }

  function diffFileRank(path: string): number {
    if (path.startsWith('specs/') || path.endsWith('.ioa.toml') || path.endsWith('.csdl.xml')) {
      return 0;
    }
    if (path.endsWith('.regression.toml') || path === 'app.toml') {
      return 1;
    }
    if (path.startsWith('policies/') || path.endsWith('.cedar')) {
      return 2;
    }
    if (path === 'APP.md' || path.startsWith('adrs/')) {
      return 3;
    }
    return 4;
  }

  function filePathFromDiffHeader(header: string): string {
    const match = header.match(/^diff --git a\/(.+?) b\/(.+)$/);
    return match?.[2] ?? header.replace(/^diff --git\s+/, '');
  }
</script>

{#if visibleFiles.length}
  <div class="grid gap-2">
    {#each visibleFiles as file (file.path)}
      <div class="overflow-hidden rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white">
        <div class="flex items-center justify-between gap-2 border-b border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5">
          <code class="truncate font-mono text-[10.5px] font-semibold text-[var(--color-ink-soft)]">{file.path}</code>
          <span class="shrink-0 font-mono text-[10px] text-[var(--color-muted)]">
            +{file.additions} -{file.deletions}
          </span>
        </div>
        <pre class="max-h-72 overflow-auto whitespace-pre-wrap break-words font-mono text-[10.5px] leading-relaxed"><code>{#each file.lines.slice(0, maxLinesPerFile) as line}<span class={[
              'block min-h-4 px-2',
              line.kind === 'add'
                ? 'bg-[#e8f7ee] text-[#176236]'
                : line.kind === 'del'
                  ? 'bg-[#fdecef] text-[#8b1e35]'
                  : line.kind === 'meta'
                    ? 'bg-[var(--color-surface-soft)] text-[var(--color-muted)]'
                    : 'text-[var(--color-ink-soft)]'
            ].join(' ')}>{line.text || ' '}</span>{/each}</code></pre>
      </div>
    {/each}
    {#if hiddenFileCount}
      <p class="rounded-[var(--radius-xs)] bg-[var(--color-surface-soft)] px-2 py-1.5 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
        {hiddenFileCount} more changed file{hiddenFileCount === 1 ? '' : 's'} in this patch
      </p>
    {/if}
  </div>
{/if}
