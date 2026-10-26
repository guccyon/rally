use crate::DomainError;

/// 通話の接続品質の生の指標（行動イベント `call.ended` の 4 属性）。通話基盤が終了時に返す。
///
/// 通話品質スコアはこの 4 つからの減点式で、式はセマンティック層の 1 か所だけに置く（docs/adr/0002 Q1）。
/// ここではスコアを計算しない。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectionQuality {
    packet_loss_rate: f64,
    jitter_ms: u32,
    round_trip_ms: u32,
    disconnect_count: u32,
}

impl ConnectionQuality {
    /// パケット損失率の上限（百分率）。
    pub const MAX_PACKET_LOSS_RATE: f64 = 100.0;

    /// `packet_loss_rate` は失われたパケットの百分率（0〜100。docs/adr/0002 Q1 の式の `packet_loss_rate(%)`）、
    /// `jitter_ms` はジッタ、`round_trip_ms` は往復の遅延（どちらもミリ秒）、`disconnect_count` は通話中に
    /// 切断した回数。損失率が 0〜100 の有限の値でなければ [`DomainError::InvalidPacketLossRate`]。
    pub fn new(
        packet_loss_rate: f64,
        jitter_ms: u32,
        round_trip_ms: u32,
        disconnect_count: u32,
    ) -> Result<Self, DomainError> {
        if !(0.0..=Self::MAX_PACKET_LOSS_RATE).contains(&packet_loss_rate) {
            return Err(DomainError::InvalidPacketLossRate);
        }
        Ok(Self {
            packet_loss_rate,
            jitter_ms,
            round_trip_ms,
            disconnect_count,
        })
    }

    /// 失われたパケットの百分率（0〜100）。
    pub const fn packet_loss_rate(&self) -> f64 {
        self.packet_loss_rate
    }

    /// ジッタ（ミリ秒）。
    pub const fn jitter_ms(&self) -> u32 {
        self.jitter_ms
    }

    /// 往復の遅延（ミリ秒）。
    pub const fn round_trip_ms(&self) -> u32 {
        self.round_trip_ms
    }

    /// 通話中に切断した回数。
    pub const fn disconnect_count(&self) -> u32 {
        self.disconnect_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_four_raw_metrics() {
        let quality = ConnectionQuality::new(1.5, 25, 120, 1).unwrap();
        assert_eq!(quality.packet_loss_rate(), 1.5);
        assert_eq!(quality.jitter_ms(), 25);
        assert_eq!(quality.round_trip_ms(), 120);
        assert_eq!(quality.disconnect_count(), 1);
    }

    #[test]
    fn packet_loss_rate_is_a_percentage() {
        ConnectionQuality::new(0.0, 0, 0, 0).unwrap();
        ConnectionQuality::new(100.0, 0, 0, 0).unwrap();
        for rate in [-0.1, 100.1, f64::NAN, f64::INFINITY] {
            assert_eq!(
                ConnectionQuality::new(rate, 0, 0, 0),
                Err(DomainError::InvalidPacketLossRate),
                "{rate}"
            );
        }
    }
}
