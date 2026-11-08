# 音

- `speaker-test.wav`: 入室前チェックの「スピーカーを試す」で鳴らす 3 秒の試し音（ド・ミ・ソ・ドの正弦波を順に重ね、減衰させたもの。16 kHz・16 bit・モノラル）。このリポジトリーで式から作ったもので、外の素材は使っていない。読むのは `src/components/coach/localMedia.ts` の `TEST_SOUND_URL`（同じオリジン。外の URL は使わない）
