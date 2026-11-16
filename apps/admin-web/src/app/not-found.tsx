import Link from "next/link";

export default function NotFound() {
  return (
    <main className="mx-auto max-w-md p-6">
      <h1 className="mb-2 text-headline font-black">ページが見つかりません</h1>
      <p className="mb-6 text-sm text-text-muted">
        開こうとしたページは見つかりませんでした。アドレスが違うか、ページが移動した可能性があります。
      </p>
      <Link href="/" className="text-primary underline">
        ログインの画面へ戻る
      </Link>
    </main>
  );
}
