<script lang="ts">
  type BrandMarkProps = {
    size?: number;
    class?: string;
  };

  let { size = 22, class: className }: BrandMarkProps = $props();
</script>

<!-- Abstract DNA: chunky 6×6 blocks crossing through a central lime base-pair -->
<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  xmlns="http://www.w3.org/2000/svg"
  shape-rendering="crispEdges"
  aria-hidden="true"
  class={className}
>
  <rect x="2"  y="2"  width="6" height="6" fill="#3D4FE3" />
  <rect x="16" y="2"  width="6" height="6" fill="#16BCE8" />
  <rect x="9"  y="9"  width="6" height="6" fill="#B7FF1A" />
  <rect x="2"  y="16" width="6" height="6" fill="#16BCE8" />
  <rect x="16" y="16" width="6" height="6" fill="#3D4FE3" />
</svg>
