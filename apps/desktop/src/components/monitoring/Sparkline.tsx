/** A tiny trend line for percentages (0-100). Decorative: the exact value is shown as text. */
export function Sparkline({
  values,
  width = 120,
  height = 28,
}: {
  values: number[];
  width?: number;
  height?: number;
}) {
  if (values.length < 2) {
    return <div style={{ width, height }} aria-hidden="true" />;
  }
  const step = width / (values.length - 1);
  const points = values
    .map((v, i) => {
      const y = height - (Math.min(100, Math.max(0, v)) / 100) * (height - 2) - 1;
      return `${(i * step).toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
      <polyline
        points={points}
        fill="none"
        stroke="var(--accent)"
        strokeWidth="1.5"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
}
