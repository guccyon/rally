//! infra の結合テストを 1 つのテストバイナリにまとめる入口（Issue #1634）。
//! tests/ のファイルごとにバイナリを作るとリンクの時間と target の容量がかさむので、各ファイルをここで `mod` として読む。
//! テストのファイルを足したら、ここにも `mod` を足す（名前の順）。

mod analytics_common_attributes;
mod analytics_event_payload;
mod domain_event_payload;
mod in_memory_domain_event_bus;
mod migration_conventions;
mod mock_ai_feedback_generator;
mod mock_alt_video_call_provider;
mod mock_push_sender;
mod mock_video_call_provider;
mod mock_video_storage;
mod object_store_analytics_sink;
mod pg_account_repository;
mod pg_announcement_draft_repository;
mod pg_announcement_repository;
mod pg_booking_repository;
mod pg_coach_acceptance_repository;
mod pg_coach_leave_repository;
mod pg_entitlement_repository;
mod pg_favorite_coach_repository;
mod pg_lesson_completion_menu_lookup;
mod pg_lesson_completion_repository;
mod pg_lesson_menu_repository;
mod pg_lesson_review_repository;
mod pg_live_lesson_session_repository;
mod pg_message_thread_repository;
mod pg_notification_repository;
mod pg_notification_settings_repository;
mod pg_slot_repository;
mod pg_video_repository;
mod schema;
mod support;
mod video_call_provider_contract;
