-- identity: コーチの自己紹介文（domain の CoachBio。admin-web の updateMyCoachProfile で書き、getMyAccount の bio で読む）。
-- コーチ 1 人に 1 つなので identity.coaches に列を足す。既存のコーチは空の自己紹介文（''）として読む。
-- 上限の文字数は domain の CoachBio::MAX_CHARS と同じ 400。判定は domain で行い、DB は壊れた値を入れないためだけに持つ。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE identity.coaches
    ADD COLUMN bio text NOT NULL DEFAULT ''
        CONSTRAINT coaches_bio_length CHECK (char_length(bio) <= 400);
