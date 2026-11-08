-- 直近の窓に積まれた衝突の続きのジョブと、そのジョブに結ぶイベントを、events.py の --events に渡す JSON の配列で出す
-- （知見 K-0014 の担保。議決 D-0076（Issue #1214 の起票時の番号は D-0074）・Issue #1214。ひな形 ci/conflict-schema.yml の conflict-record のジョブが
-- 制御プレーンの events に対して走らせ、events.py --record-only の rule missing_conflict_files で照らす）。
-- 出すのは、occurred_at が :since（Engine に engine/conflict-files.patch を当てた時刻）と直近 :window_days 日の始まりの遅いほうから後の
-- continue.enqueued（reason=conflict）と、同じ job_id の job.started・job.finished・conflict.files（窓の前でも出す。積んだ時点より前か後かは
-- events.py がイベント列の順（id）で見る）。差分の前の Engine は積むときに衝突したファイルを載せないので、前の回を照らすとすべて落ちる。
--   psql "$DATABASE_URL" -X -q -A -t -v ON_ERROR_STOP=1 -v since=<時刻> -v window_days=<日数> -f tools/conflict-check/record-events.sql
with enqueued as (
  select e.job_id
  from events e
  where e.name = 'continue.enqueued'
    and e.payload ->> 'reason' = 'conflict'
    and e.job_id is not null
    and e.occurred_at >= greatest(:'since'::timestamptz, now() - make_interval(days => :'window_days'::int))
)
select coalesce(json_agg(json_build_object('id', e.id, 'name', e.name, 'occurred_at', e.occurred_at, 'job_id', e.job_id,
                                           'correlation_id', e.correlation_id, 'payload', e.payload) order by e.id), '[]')
from events e
where (e.name = 'continue.enqueued'
       and e.payload ->> 'reason' = 'conflict'
       and e.occurred_at >= greatest(:'since'::timestamptz, now() - make_interval(days => :'window_days'::int)))
   or (e.name in ('job.started', 'job.finished', 'conflict.files') and e.job_id in (select job_id from enqueued));
