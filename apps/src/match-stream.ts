export function matchStream<T>(signal?: AbortSignal, onClose?: () => void) {
  const pending: T[] = [];
  let wake: (() => void) | undefined;
  let closed = false;
  let finished = false;
  function close() {
    if (closed) return;
    closed = true;
    signal?.removeEventListener("abort", close);
    onClose?.();
    wake?.();
  }
  if (signal?.aborted) close();
  else signal?.addEventListener("abort", close, { once: true });
  return {
    close,
    finish() {
      finished = true;
      wake?.();
    },
    push(value: T) {
      if (closed || finished) return;
      pending.push(value);
      wake?.();
    },
    async *updates() {
      try {
        while (!closed) {
          if (pending.length) {
            const next = pending.shift();
            if (next !== undefined) yield next;
          } else {
            if (finished) break;
            await new Promise<void>(resolve => { wake = resolve; });
            wake = undefined;
          }
        }
      } finally { close(); }
    },
  };
}
