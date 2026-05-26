<script lang="ts">
  // Low-bandwidth director controls. Three operations only:
  //   - Pick winner  (only valid in Selecting)
  //   - Approve      (only valid in AwaitingApproval; gated by autonomy)
  //   - Revert       (only valid in Live)
  // Each button POSTs a bound Evo.DE action against /tdata/Evolutions.
  // The fitness function is set elsewhere (via natural language) — the
  // UI deliberately does not expose a fitness editor.

  import { Button } from '$lib/components/ui';
  import { CheckCircle2, Crown, RotateCcw } from '@lucide/svelte';
  import type { Evolution } from '$lib/studio';

  type Props = {
    evolution: Evolution;
    selectedWinnerId: string;
    busy: boolean;
    onApprove: () => void;
    onRevert: () => void;
    onConfirmWinner: () => void;
  };

  let { evolution, selectedWinnerId, busy, onApprove, onRevert, onConfirmWinner }: Props = $props();

  const inSelecting = $derived(evolution.status === 'Selecting');
  const inAwaitingApproval = $derived(evolution.status === 'AwaitingApproval');
  const inLive = $derived(evolution.status === 'Live');
</script>

<div class="flex flex-wrap items-center gap-2">
  <Button
    variant="primary"
    size="md"
    disabled={!inSelecting || !selectedWinnerId || busy}
    onclick={onConfirmWinner}
    title={!inSelecting ? 'Pick winner is only valid in Selecting' : (selectedWinnerId ? `Confirm ${selectedWinnerId.slice(0,12)}…` : 'Click a variant header to choose')}
  >
    <Crown size={12} />
    Pick winner
  </Button>

  <Button
    variant="accent"
    size="md"
    disabled={!inAwaitingApproval || busy}
    onclick={onApprove}
    title={inAwaitingApproval ? 'Approve for merge + hot-deploy' : 'Approve is only valid in AwaitingApproval'}
  >
    <CheckCircle2 size={12} />
    Approve
  </Button>

  <Button
    variant="outline"
    size="md"
    disabled={!inLive || busy}
    onclick={onRevert}
    title={inLive ? 'Revert the live deploy' : 'Revert is only valid for Live evolutions'}
  >
    <RotateCcw size={12} />
    Revert
  </Button>
</div>
