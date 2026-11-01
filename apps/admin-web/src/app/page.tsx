import { LoginForm } from "@/components/LoginForm";

export default function LoginPage() {
  return (
    <main className="mx-auto max-w-md p-6">
      <h1 className="mb-2 text-xl font-bold">Rally 管理画面にログイン</h1>
      <p className="mb-6 text-sm text-secondary">
        Gen1 は本物の認証がありません。アカウント登録で受け取ったモックのアクセストークンを入力してください。
      </p>
      <LoginForm />
    </main>
  );
}
