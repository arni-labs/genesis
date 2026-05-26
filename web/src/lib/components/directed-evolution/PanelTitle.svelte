<script lang="ts">
  import type { Component } from 'svelte';

  type PanelTitleProps = {
    icon: Component;
    title: string;
  };

  let { icon, title }: PanelTitleProps = $props();
  let Icon = $derived(icon);
</script>

<div class="flex items-center gap-1.5">
  <Icon size={13} class="text-[var(--color-primary)]" />
  <p class="v-eyebrow">{title}</p>
</div>
