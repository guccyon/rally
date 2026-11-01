/**
 * しるし（specs/design/components.yaml の badge）。未読の数か点。
 * 入口のラベルにも数を足す（「チャット、未読 3 件」）ので、ここは読み上げない。
 */
export function Badge({ count }: { count?: number }) {
  if (count === undefined) {
    return <span aria-hidden="true" data-testid="badge-dot" className="inline-block size-2 rounded-pill bg-danger" />;
  }
  return (
    <span
      aria-hidden="true"
      className="inline-flex h-5 min-w-5 items-center justify-center rounded-sm bg-danger px-1 text-xs font-bold text-on-primary"
    >
      {count}
    </span>
  );
}
