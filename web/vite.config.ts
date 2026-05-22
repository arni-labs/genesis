import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

const proxyTarget = process.env.VITE_TEMPER_API_PROXY ?? 'http://127.0.0.1:3231';

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  server: {
    proxy: {
      '/tdata': proxyTarget
    }
  }
});
