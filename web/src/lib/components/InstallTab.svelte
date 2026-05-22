<script lang="ts">
  import { AlertCircle, Copy, GitCommitHorizontal, PackageCheck } from '@lucide/svelte';
  import { IconButton } from '$lib/components/ui';
  import type { RegistryApp } from '$lib/types';

  type InstallCard = {
    title: string;
    icon: typeof PackageCheck;
    command: string;
    copyLabel: string;
    description: string;
  };

  type InstallTabProps = {
    app: RegistryApp;
    odata: string;
    cli: string;
    paw: string;
    clone: string;
    warnings: string[];
    onCopy: (value: string, label: string) => void;
  };

  let { app, odata, cli, paw, clone, warnings, onCopy }: InstallTabProps = $props();

  const cards = $derived<InstallCard[]>([
    {
      title: 'OData action',
      icon: PackageCheck,
      command: odata,
      copyLabel: 'OData install command',
      description: 'Spec-owned install surface for pinned Genesis app bytes.'
    },
    {
      title: 'Temper CLI',
      icon: PackageCheck,
      command: cli,
      copyLabel: 'Temper CLI install command',
      description: 'CLI wrapper around the same App.Install OData action.'
    },
    {
      title: 'TemperPaw tool',
      icon: PackageCheck,
      command: paw,
      copyLabel: 'TemperPaw install call',
      description: 'Tool path for an agent to request the same pinned app install.'
    },
    {
      title: 'Clone',
      icon: GitCommitHorizontal,
      command: clone,
      copyLabel: 'Clone command',
      description: 'Smart HTTP reconstructs this repository from Temper objects.'
    }
  ]);
</script>

<div class="grid gap-3 px-3 pb-3 pt-3">
  <div class="grid gap-2 lg:grid-cols-2">
    {#each cards as card (card.title)}
      <div class="rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white px-3 py-2.5">
        <div class="flex items-center justify-between gap-2">
          <p class="v-eyebrow">{card.title}</p>
          <card.icon size={12} class="text-[var(--color-primary)]" />
        </div>
        <div
          class="mt-1.5 flex items-center justify-between gap-2 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] py-1 pl-2.5 pr-1"
        >
          <code class="flex-1 truncate font-mono text-[11px] text-[var(--color-ink-soft)]">
            {card.command}
          </code>
          <IconButton
            aria-label={`Copy ${card.copyLabel}`}
            class="h-6 w-6"
            onclick={() => onCopy(card.command, card.copyLabel)}
          >
            <Copy size={11} />
          </IconButton>
        </div>
        <p class="mt-1.5 font-sans text-[11.5px] leading-snug text-[var(--color-muted)]">
          {card.description}
        </p>
      </div>
    {/each}
  </div>

  {#if warnings.length}
    <ul class="grid gap-1">
      {#each warnings as warning, index (`${warning}-${index}`)}
        <li class="flex items-start gap-2 rounded-[var(--radius-md)] border border-[var(--color-error)]/30 bg-[rgba(217,45,75,0.08)] px-3 py-1.5 font-sans text-[12px] text-[#7a1830]">
          <AlertCircle size={12} class="mt-[2px] shrink-0" />
          <span>{warning}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>
