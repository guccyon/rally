/** 読み込み中（components.yaml の loading）。 */
export function Loading({ label = "読み込み中…" }: { label?: string }) {
  return <p className="text-sm text-gray-500">{label}</p>;
}
