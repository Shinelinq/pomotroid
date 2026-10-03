// Browser-only synthetic data. This module is never loaded by the packaged app.
export function dataFixture({ items, rows, timer, today, params }) {
  timer.has_started = false;
  timer.is_running = false;
  timer.is_paused = false;
  items.push(
    { id: 5, name: '很长的分类名称用来验证省略和完整名称提示', archived: false },
    { id: 6, name: '学习', archived: false },
    { id: 7, name: '杂项', archived: false }
  );
  rows.splice(
    0,
    rows.length,
    ...[21600, 10800, 3596, 1, 1, 1, 1].map((secs, i) => ({
      id: i + 1,
      date: today,
      category: i === 2 ? null : [1, 2, 4, 3, 5, 6, 7][i],
      secs,
      completed: true,
      started_at: new Date(`${today}T08:00:00`).getTime() / 1000 + i,
      ended_at: null,
    }))
  );
  let number = 0,
    active = '',
    options = { history: true, profiles: true, preferences: false };
  let last = null;
  let scope;
  let committed = false;
  const preview = () => ({
    token: active,
    filename: 'synthetic.pomotroid.json',
    exported_at: '2026-10-03T00:00:00Z',
    app_version: '1.9.0',
    source_timezone: 'Asia/Shanghai',
    start: rows[0].started_at,
    end: rows[0].started_at,
    has_profiles: true,
    has_preferences: true,
    duplicates: 2,
    options: { ...options },
    summary: {
      sessions: {
        added: options.history && !committed ? 100 : 0,
        existing: options.history ? 20 : 0,
        conflicts: options.history ? 1 : 0,
      },
      categories: { added: options.history && !committed ? 2 : 0, existing: 0, conflicts: 0 },
      profiles: { added: options.profiles && !committed ? 1 : 0, existing: 0, conflicts: 0 },
      preferences: options.preferences,
    },
    conflicts: options.history ? 1 : 0,
    renames: options.history ? 1 : 0,
    digest: 'test-only',
  });
  return async (command, args) => {
    let value;
    switch (command) {
      case 'data_begin':
        active = `test-${++number}`;
        value = active;
        break;
      case 'data_cancel':
        break;
      case 'data_last_export':
        value = last;
        break;
      case 'data_read':
        if (params.has('cancel')) value = null;
        else if (params.has('invalid')) throw 'data_version';
        else value = preview();
        break;
      case 'data_replan':
        options = args.options;
        await new Promise((r) => setTimeout(r, 100));
        value = preview();
        break;
      case 'data_details':
        value = {
          conflicts: args.renames
            ? []
            : [
                {
                  kind: 'session',
                  label: String(rows[0].started_at),
                  differences: [{ field: 'completed', local: 'false', file: 'true' }],
                },
              ],
          renames: args.renames ? [{ kind: 'category', from: '阅读', to: '阅读 (导入)' }] : [],
        };
        break;
      case 'data_commit':
        if (timer.has_started) throw 'data_active_round';
        committed = true;
        value = {
          committed: true,
          refresh_failed: false,
          preview: null,
          summary: preview().summary,
        };
        break;
      case 'data_export':
        if (params.has('cancel')) value = null;
        else {
          last = new Date().toISOString();
          value = {
            filename: 'synthetic.pomotroid.json',
            rows: 100,
            categories: 2,
            profiles: args.profiles ? 1 : 0,
            warning: false,
          };
        }
        break;
      case 'report_preview':
        scope = args.scope;
        value = {
          scope,
          rows: 125,
          focus_secs: 36000,
          filename: 'synthetic.csv',
          timezone: 'Asia/Shanghai',
        };
        break;
      case 'report_save':
        value = params.has('cancel')
          ? null
          : { filename: 'synthetic.csv', rows: 125, categories: 0, profiles: 0, warning: false };
        break;
      case 'stats_distribution': {
        if (params.has('distribution-error')) throw 'test error';
        const grouped = new Map();
        for (const row of rows.filter(
          (r) => r.completed && r.date >= args.start && r.date <= args.end
        )) {
          const c = items.find((c) => c.id === row.category);
          const v = grouped.get(row.category) ?? {
            category_id: row.category,
            name: c?.name ?? null,
            archived: c?.archived ?? false,
            completed: 0,
            focus_secs: 0,
          };
          v.completed++;
          v.focus_secs += row.secs;
          grouped.set(row.category, v);
        }
        const values = [...grouped.values()].sort(
          (a, b) => b.focus_secs - a.focus_secs || String(a.name).localeCompare(String(b.name))
        );
        value = { rows: values, total_focus_secs: values.reduce((n, r) => n + r.focus_secs, 0) };
        break;
      }
      default:
        return { handled: false };
    }
    return { handled: true, value };
  };
}
