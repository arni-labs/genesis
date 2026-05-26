// Studio is a client-side single-page view. Prerendering would
// freeze a snapshot of the data plane at build time, which is the
// opposite of what we want here. We inherit ssr=false from the root
// layout but override prerender locally.

export const prerender = false;
export const ssr = false;
