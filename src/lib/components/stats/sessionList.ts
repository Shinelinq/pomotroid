import type {
  SessionQuery,
  SessionPage,
  SessionRecord,
  SessionSummary,
  SessionCursor,
} from '../../types';

export interface SessionListState {
  records: SessionRecord[];
  summary: SessionSummary | null;
  cursor: SessionCursor | null;
  loading: boolean;
  loadingMore: boolean;
  error: { operation: 'refresh' | 'more'; message: string } | null;
}
export const emptySessionList = (): SessionListState => ({
  records: [],
  summary: null,
  cursor: null,
  loading: false,
  loadingMore: false,
  error: null,
});

/** One query range, bounded pages, no timer or persistent history state. */
export function createSessionList(
  read: (query: SessionQuery) => Promise<SessionPage>,
  changed: (state: SessionListState) => void
) {
  let state = emptySessionList();
  let scope: SessionQuery | null = null;
  let generation = 0;
  let disposed = false;
  function update(patch: Partial<SessionListState>) {
    state = { ...state, ...patch };
    if (!disposed) changed(state);
  }
  const current = (ticket: number) => !disposed && generation === ticket;
  async function reload(reset: boolean) {
    if (!scope || disposed) return;
    const query = scope;
    const count = reset ? 50 : Math.max(50, state.records.length + (state.loadingMore ? 50 : 0));
    const ticket = ++generation;
    update({
      ...(reset ? emptySessionList() : {}),
      loading: true,
      loadingMore: false,
      error: null,
    });
    try {
      let cursor: SessionCursor | null = null;
      let summary: SessionSummary | null = null;
      const records: SessionRecord[] = [];
      do {
        const page = await read({ ...query, cursor, limit: 50 });
        if (!current(ticket)) return;
        records.push(...page.records);
        summary = page.summary;
        cursor = page.next_cursor;
      } while (cursor && records.length < count);
      update({ records, summary, cursor });
    } catch (error) {
      if (current(ticket)) update({ error: { operation: 'refresh', message: String(error) } });
    } finally {
      if (current(ticket)) update({ loading: false });
    }
  }
  async function more() {
    if (!scope || !state.cursor || state.loading || state.loadingMore || disposed) return;
    const ticket = generation;
    const query = { ...scope, cursor: state.cursor, limit: 50 };
    update({ loadingMore: true, error: null });
    try {
      const page = await read(query);
      if (!current(ticket)) return;
      const ids = new Set(state.records.map((row) => row.id));
      update({
        records: [...state.records, ...page.records.filter((row) => !ids.has(row.id))],
        summary: page.summary,
        cursor: page.next_cursor,
      });
    } catch (error) {
      if (current(ticket)) update({ error: { operation: 'more', message: String(error) } });
    } finally {
      if (current(ticket)) update({ loadingMore: false });
    }
  }
  return {
    open(query: SessionQuery) {
      scope = { date: query.date, hour: query.hour ?? null, filter: { ...query.filter } };
      return reload(true);
    },
    refresh: () => reload(false),
    more,
    retry: () =>
      state.error?.operation === 'more' && !state.error.message.includes('detail_invalid_cursor')
        ? more()
        : reload(false),
    snapshot: () => state,
    dispose() {
      disposed = true;
      ++generation;
    },
  };
}
