import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig, type ProxyOptions } from 'vite';

const proxyTarget = process.env.VITE_TEMPER_API_PROXY ?? 'http://127.0.0.1:3231';
const temperApiProxy: ProxyOptions = {
  target: proxyTarget,
  changeOrigin: true,
  timeout: 0,
  proxyTimeout: 0
};

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  server: {
    proxy: {
      '/tdata': temperApiProxy,
      '/observe': temperApiProxy,
      '/api': temperApiProxy
    }
  }
});
