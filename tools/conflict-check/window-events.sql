-- 直近の窓の、衝突したファイルの列（conflict_files）を照らすイベントを、schema.py の --events に渡す JSON の配列で出す
-- （知見 K-0014 の担保。議決 D-0101・Issue #1207。ひな形 ci/conflict-schema.yml が制御プレーンの events に対して走らせる）。
-- 照らすイベントは conflict-files.yaml の events と同じ 3 つ。occurred_at が :since（Engine に engine/conflict-files.patch を当てた時刻）と
-- 直近 :window_days 日の始まりの遅いほうより前のものは出さない（差分の前の Engine はこの欄を出さないので、照らすとすべて missing_field で落ちる）。
--   psql "$DATABASE_URL" -X -q -A -t -v ON_ERROR_STOP=1 -v since=<時刻> -v window_days=<日数> -f tools/conflict-check/window-events.sql
select coalesce(json_agg(json_build_object('id', e.id, 'name', e.name, 'occurred_at', e.occurred_at, 'job_id', e.job_id,
                                           'correlation_id', e.correlation_id, 'payload', e.payload) order by e.id), '[]')
from events e
where e.name in ('pr.conflicted', 'continue.enqueued', 'handoff.saved')
  and e.occurred_at >= greatest(:'since'::timestamptz, now() - make_interval(days => :'window_days'::int));
