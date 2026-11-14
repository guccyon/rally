import { LoginForm } from "@/components/LoginForm";

export default function LoginPage() {
  return (
    <main className="mx-auto max-w-md p-6">
      <h1 className="mb-2 text-headline font-black">Rally 管理画面にログイン</h1>
      <p className="mb-6 text-sm text-text-muted">
        アカウントを登録したときに受け取ったアクセストークンを、下の欄に貼り付けてください。使うロール（コーチ／本部）も選んでください。
      </p>
      <LoginForm />
    </main>
  );
}
