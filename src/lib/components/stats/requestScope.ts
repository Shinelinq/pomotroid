/** Both statistics endpoints share one scope; replacing it invalidates results,
 * errors and loading completions from every outstanding request. */
export function createRequestScope() {
  let generation = 0;
  const latest = { detailed: 0, heatmap: 0 };
  return {
    begin(channel: keyof typeof latest) {
      const scope = generation;
      const request = ++latest[channel];
      return () => scope === generation && latest[channel] === request;
    },
    invalidate() {
      ++generation;
    },
  };
}
