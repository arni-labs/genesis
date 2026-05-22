import type { Highlighter, BundledLanguage, BundledTheme } from 'shiki';

const langMap: Record<string, BundledLanguage> = {
  ts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  jsx: 'jsx',
  mjs: 'javascript',
  cjs: 'javascript',
  rs: 'rust',
  toml: 'toml',
  json: 'json',
  jsonc: 'jsonc',
  md: 'markdown',
  markdown: 'markdown',
  yaml: 'yaml',
  yml: 'yaml',
  py: 'python',
  go: 'go',
  sh: 'shellscript',
  bash: 'shellscript',
  zsh: 'shellscript',
  css: 'css',
  scss: 'scss',
  html: 'html',
  htm: 'html',
  svelte: 'svelte',
  vue: 'vue',
  xml: 'xml',
  sql: 'sql',
  dockerfile: 'docker'
};

const SUPPORTED_LANGS: BundledLanguage[] = [
  'typescript',
  'tsx',
  'javascript',
  'jsx',
  'rust',
  'toml',
  'json',
  'jsonc',
  'markdown',
  'yaml',
  'python',
  'go',
  'shellscript',
  'css',
  'scss',
  'html',
  'svelte',
  'vue',
  'xml',
  'sql',
  'docker'
];

const THEME: BundledTheme = 'github-light';

let highlighterPromise: Promise<Highlighter> | null = null;

async function getHighlighter(): Promise<Highlighter> {
  if (!highlighterPromise) {
    highlighterPromise = (async () => {
      const { createHighlighter } = await import('shiki');
      return createHighlighter({
        themes: [THEME],
        langs: SUPPORTED_LANGS
      });
    })();
  }
  return highlighterPromise;
}

export function detectLanguage(path: string): string {
  if (!path) {
    return 'text';
  }

  const lower = path.toLowerCase();
  const baseName = lower.split('/').pop() ?? '';

  if (baseName === 'dockerfile' || baseName.startsWith('dockerfile.')) {
    return 'docker';
  }
  if (baseName === 'makefile') {
    return 'shellscript';
  }

  const dot = baseName.lastIndexOf('.');
  if (dot < 0) {
    return 'text';
  }
  const ext = baseName.slice(dot + 1);
  return langMap[ext] ?? 'text';
}

export async function highlight(code: string, language: string): Promise<string> {
  if (!code) {
    return '';
  }
  const lang = SUPPORTED_LANGS.includes(language as BundledLanguage) ? language : 'text';
  try {
    const highlighter = await getHighlighter();
    return highlighter.codeToHtml(code, {
      lang: lang as BundledLanguage | 'text',
      theme: THEME
    });
  } catch {
    const escaped = code
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
    return `<pre><code>${escaped}</code></pre>`;
  }
}

export function preferLanguage(path: string, fallback = 'text'): string {
  const detected = detectLanguage(path);
  return detected === 'text' ? fallback : detected;
}
