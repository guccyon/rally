/** 操作の結果の 1 行。失敗は role="alert"。 */
export function ResultMessage({ result }: { result: { ok: boolean; message: string } | null }) {
  if (!result) return null;
  return result.ok ? (
    <p role="status" className="text-success">
      {result.message}
    </p>
  ) : (
    <p role="alert" className="text-danger">
      {result.message}
    </p>
  );
}
