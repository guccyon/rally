use super::{MessageAuthor, MessageBody, MessageKind, MessageSender};
use crate::async_lesson::{VideoId, VideoReviewId};

/// メッセージの中身。種類（[`MessageKind`]）ごとに持つものを型で分ける（本文を持つのは本文の種類だけ）。
///
/// 添削依頼・添削の返却は async-lesson の動画・添削を ID で指すだけで、添削の有無・回数・状態の写しは持たない
/// （それらは async-lesson の持ち物）。スタンプや AI コンシェルジュの発言は、変種を足して表す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageContent {
    /// 本文（当事者どうしの会話）
    Text(MessageBody),
    /// 添削依頼。生徒が動画 `video_id` を添削依頼として送った（送り手は生徒）
    VideoReviewRequest { video_id: VideoId },
    /// 添削の返却。コーチが動画 `video_id` に添削 `review_id` を返した（送り手はコーチ）
    VideoReviewReturn {
        video_id: VideoId,
        review_id: VideoReviewId,
    },
}

impl MessageContent {
    pub const fn kind(&self) -> MessageKind {
        match self {
            MessageContent::Text(_) => MessageKind::Text,
            MessageContent::VideoReviewRequest { .. } => MessageKind::VideoReviewRequest,
            MessageContent::VideoReviewReturn { .. } => MessageKind::VideoReviewReturn,
        }
    }

    /// 本文。本文の種類でなければ `None`。
    pub const fn body(&self) -> Option<&MessageBody> {
        match self {
            MessageContent::Text(body) => Some(body),
            MessageContent::VideoReviewRequest { .. }
            | MessageContent::VideoReviewReturn { .. } => None,
        }
    }

    /// 指している動画。本文の種類なら `None`。
    pub const fn video_id(&self) -> Option<VideoId> {
        match self {
            MessageContent::Text(_) => None,
            MessageContent::VideoReviewRequest { video_id }
            | MessageContent::VideoReviewReturn { video_id, .. } => Some(*video_id),
        }
    }

    /// 指している添削。添削の返却でなければ `None`。
    pub const fn review_id(&self) -> Option<VideoReviewId> {
        match self {
            MessageContent::VideoReviewReturn { review_id, .. } => Some(*review_id),
            MessageContent::Text(_) | MessageContent::VideoReviewRequest { .. } => None,
        }
    }

    /// 送り手 `author` がこの種類のメッセージを流せるか。本文は誰でも（システムのお知らせも本文）、添削依頼は生徒、
    /// 添削の返却はコーチだけ。
    pub const fn allows(&self, author: MessageAuthor) -> bool {
        match self {
            MessageContent::Text(_) => true,
            MessageContent::VideoReviewRequest { .. } => {
                matches!(author, MessageAuthor::Participant(MessageSender::Student))
            }
            MessageContent::VideoReviewReturn { .. } => {
                matches!(author, MessageAuthor::Participant(MessageSender::Coach))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIDEO: VideoId = VideoId::from_u128(5);
    const REVIEW: VideoReviewId = VideoReviewId::from_u128(6);
    const STUDENT: MessageAuthor = MessageAuthor::Participant(MessageSender::Student);
    const COACH: MessageAuthor = MessageAuthor::Participant(MessageSender::Coach);

    #[test]
    fn only_the_text_kind_has_a_body() {
        let body = MessageBody::new("こんにちは").unwrap();
        let text = MessageContent::Text(body.clone());
        assert_eq!(text.kind(), MessageKind::Text);
        assert_eq!(text.body(), Some(&body));
        assert_eq!((text.video_id(), text.review_id()), (None, None));

        let request = MessageContent::VideoReviewRequest { video_id: VIDEO };
        assert_eq!(request.kind(), MessageKind::VideoReviewRequest);
        assert_eq!(request.body(), None);
        assert_eq!(
            (request.video_id(), request.review_id()),
            (Some(VIDEO), None)
        );

        let returned = MessageContent::VideoReviewReturn {
            video_id: VIDEO,
            review_id: REVIEW,
        };
        assert_eq!(returned.kind(), MessageKind::VideoReviewReturn);
        assert_eq!(returned.body(), None);
        assert_eq!(
            (returned.video_id(), returned.review_id()),
            (Some(VIDEO), Some(REVIEW))
        );
    }

    #[test]
    fn the_request_is_from_the_student_and_the_return_is_from_the_coach() {
        let text = MessageContent::Text(MessageBody::new("x").unwrap());
        let request = MessageContent::VideoReviewRequest { video_id: VIDEO };
        let returned = MessageContent::VideoReviewReturn {
            video_id: VIDEO,
            review_id: REVIEW,
        };
        for author in MessageAuthor::ALL {
            assert!(text.allows(author), "{author:?}");
        }
        assert_eq!(
            MessageAuthor::ALL.map(|a| request.allows(a)),
            [true, false, false]
        );
        assert_eq!(
            MessageAuthor::ALL.map(|a| returned.allows(a)),
            [false, true, false]
        );
        assert!(request.allows(STUDENT) && returned.allows(COACH));
    }
}
