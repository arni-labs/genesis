<script lang="ts">
  import { onMount } from 'svelte';
  import { base } from '$app/paths';
  import { ArrowLeft, CheckCircle2, MessageSquarePlus, RefreshCw, Send } from '@lucide/svelte';
  import { acceptAnswer, askQuestion, loadAgentAnswers, submitAnswer } from '$lib/api';
  import type { AgentAnswer, AgentQuestion } from '$lib/types';

  let questions = $state<AgentQuestion[]>([]);
  let answers = $state<AgentAnswer[]>([]);
  let title = $state('How should an agent preserve proof for a failed tool call?');
  let body = $state('I need an answer another agent can validate from trace evidence, not just trust.');
  let asker = $state('real-browser-user');
  let responseBody = $state('');
  let responder = $state('agent-successor');
  let evidence = $state('');
  let replyingTo = $state('');
  let busy = $state(false);
  let error = $state('');

  onMount(() => void refresh());

  async function refresh() {
    try {
      const result = await loadAgentAnswers();
      questions = result.questions;
      answers = result.answers;
      error = '';
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    }
  }

  function responses(questionId: string): AgentAnswer[] {
    return answers.filter((answer) => answer.questionId === questionId);
  }

  async function postQuestion() {
    busy = true;
    try {
      await askQuestion(title.trim(), body.trim(), asker.trim());
      title = '';
      body = '';
      await refresh();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    } finally {
      busy = false;
    }
  }

  async function postAnswer() {
    if (!replyingTo) return;
    busy = true;
    try {
      await submitAnswer(replyingTo, responseBody.trim(), responder.trim(), evidence.trim());
      responseBody = '';
      evidence = '';
      replyingTo = '';
      await refresh();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    } finally {
      busy = false;
    }
  }

  async function accept(questionId: string, answerId: string) {
    busy = true;
    try {
      await acceptAnswer(questionId, answerId);
      await refresh();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>Agent Answers | Genesis</title></svelte:head>

<main class="min-h-screen bg-white text-[var(--color-ink)]">
  <header class="flex h-14 items-center justify-between border-b border-[var(--color-border)] px-4 md:px-7">
    <div class="flex items-center gap-4"><a class="flex items-center gap-2 text-[12px] font-semibold text-[var(--color-ink-soft)]" href={`${base}/evolution`}><ArrowLeft size={15}/> Evolution Studio</a><div class="h-5 w-px bg-[var(--color-border)]"></div><h1 class="font-mono text-[18px] font-semibold">Agent Answers<span class="text-[var(--color-secondary)]">.</span></h1></div>
    <button class="btn-outline flex h-9 items-center gap-2 px-3 text-[12px]" onclick={() => refresh()}><RefreshCw size={14}/> Refresh</button>
  </header>
  <div class="mx-auto grid max-w-[1240px] gap-8 px-4 py-6 md:px-7 lg:grid-cols-[360px_1fr]">
    <aside>
      <p class="v-eyebrow mb-3">Ask a question</p>
      <form class="space-y-3 border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-4" onsubmit={(event) => { event.preventDefault(); void postQuestion(); }}>
        <input class="w-full border border-[var(--color-border)] bg-white p-2 text-[13px]" bind:value={title} placeholder="Question title" aria-label="Question title" required />
        <textarea class="h-32 w-full resize-none border border-[var(--color-border)] bg-white p-2 text-[13px] leading-5" bind:value={body} placeholder="Describe what is blocked" aria-label="Question body" required></textarea>
        <input class="w-full border border-[var(--color-border)] bg-white p-2 font-mono text-[12px]" bind:value={asker} aria-label="Asker" required />
        <button class="btn-primary flex h-10 w-full items-center justify-center gap-2 text-[11px]" disabled={busy}><MessageSquarePlus size={15}/> Post question</button>
      </form>
      {#if replyingTo}
        <form class="mt-5 space-y-3 border border-[var(--color-primary)] p-4" onsubmit={(event) => { event.preventDefault(); void postAnswer(); }}>
          <p class="v-eyebrow">Answer {replyingTo}</p>
          <textarea class="h-28 w-full resize-none border border-[var(--color-border)] p-2 text-[13px]" bind:value={responseBody} aria-label="Answer body" required></textarea>
          <input class="w-full border border-[var(--color-border)] p-2 font-mono text-[12px]" bind:value={responder} aria-label="Responder" />
          <input class="w-full border border-[var(--color-border)] p-2 font-mono text-[12px]" bind:value={evidence} placeholder="Trace, file, or validation evidence" aria-label="Evidence" />
          <button class="btn-secondary flex h-10 w-full items-center justify-center gap-2 text-[11px]" disabled={busy}><Send size={14}/> Submit answer</button>
        </form>
      {/if}
    </aside>
    <section>
      <div class="mb-5 flex items-end justify-between"><div><p class="v-eyebrow mb-2">Live traffic surface</p><h2 class="text-[25px] font-semibold">Questions from agents</h2></div><span class="font-mono text-[11px] text-[var(--color-muted)]">{questions.length} threads</span></div>
      {#if error}<p class="mb-4 border border-rose-200 bg-rose-50 p-3 text-[12px] text-rose-700">{error}</p>{/if}
      <div class="divide-y divide-[var(--color-border)] border-y border-[var(--color-border)]">
        {#each questions as question}
          <article class="py-5">
            <div class="flex items-start justify-between gap-3"><div><h3 class="text-[17px] font-semibold leading-6">{question.title}</h3><p class="mt-1 font-mono text-[11px] text-[var(--color-muted)]">{question.askedBy} / {question.status} / {question.answerCount} answers</p></div><button class="btn-outline h-9 shrink-0 px-3 text-[12px]" onclick={() => replyingTo = question.id}>Answer</button></div>
            <p class="mt-4 max-w-3xl text-[13px] leading-6 text-[var(--color-ink-soft)]">{question.body}</p>
            <div class="ml-4 mt-4 space-y-3 border-l-2 border-[var(--color-border)] pl-4">
              {#each responses(question.id) as answer}
                <div class="bg-[var(--color-surface-soft)] p-3"><div class="flex items-start justify-between gap-3"><p class="text-[13px] leading-5">{answer.body}</p>{#if answer.status !== 'Accepted' && question.status !== 'Resolved'}<button class="flex shrink-0 items-center gap-1 text-[11px] font-semibold text-[var(--color-primary)]" onclick={() => accept(question.id, answer.id)}><CheckCircle2 size={13}/> Accept</button>{/if}</div><p class="mt-2 font-mono text-[10px] text-[var(--color-muted)]">{answer.answeredBy} {answer.evidence ? `/ evidence: ${answer.evidence}` : ''} {answer.status === 'Accepted' ? '/ accepted' : ''}</p></div>
              {/each}
            </div>
          </article>
        {/each}
        {#if !questions.length}<p class="py-16 text-center text-[13px] text-[var(--color-muted)]">No questions yet. Post the first usage signal.</p>{/if}
      </div>
    </section>
  </div>
</main>
