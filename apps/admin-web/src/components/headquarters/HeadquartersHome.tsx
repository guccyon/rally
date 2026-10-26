import { NotImplemented, Section } from "@/components/Section";

/** 本部の画面（読むだけ）。一覧の API が admin-web の契約にまだないので「未実装」と出す。 */
export function HeadquartersHome() {
  return (
    <>
      <Section title="予約の一覧">
        <NotImplemented>予約の一覧を取る API が admin-web の契約にまだありません。</NotImplemented>
      </Section>
      <Section title="完了記録の一覧">
        <NotImplemented>完了したレッスンの一覧を取る API が admin-web の契約にまだありません。</NotImplemented>
      </Section>
    </>
  );
}
