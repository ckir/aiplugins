//! `pair_status` and the liveness pair shared with `service_status` and `session_wait`.

use serde_json::{json, Value};

use crate::clock::Clock;
use crate::engine::Engine;
use crate::error::{ErrorCode, ToolError, ToolResult};
use crate::liveness::{side_liveness, Liveness, Side};
use crate::pairing::read_record;
use crate::transport::Transport;

use super::PairStatusArgs;

impl<T: Transport, C: Clock> Engine<T, C> {
    /// `(tui, claude)` re-read from disk: `none` for both without a pair id, `unknown` for both when
    /// the record is missing or unreadable.
    pub fn pair_liveness(&self) -> (Liveness, Liveness) {
        let Some(id) = self.env.pair.as_deref() else {
            return (Liveness::None, Liveness::None);
        };
        match read_record(&self.env.home, id) {
            Ok(Some(r)) => {
                let now = self.clock().now_ms();
                let side = |s| side_liveness(s, &r, &self.env.home, &*self.probe, now);
                (side(Side::Tui), side(Side::Controller))
            }
            _ => (Liveness::Unknown, Liveness::Unknown),
        }
    }

    pub async fn pair_status(&self, _a: PairStatusArgs) -> ToolResult {
        let Some(id) = self.env.pair.as_deref() else {
            return Ok(json!({"paired": null}));
        };
        let record = read_record(&self.env.home, id)?.ok_or_else(|| {
            ToolError::new(
                ErrorCode::PairNotFound,
                format!("no pair record for {id}"),
            )
            .hint("restart the controller with `opencode-pair resume <pair-id>`, or pair again with `opencode-pair`")
        })?;
        let now = self.clock().now_ms();
        let mut out = serde_json::to_value(&record)
            .map_err(|e| ToolError::new(ErrorCode::IoError, e.to_string()))?;
        let tui = side_liveness(Side::Tui, &record, &self.env.home, &*self.probe, now);
        let claude = side_liveness(Side::Controller, &record, &self.env.home, &*self.probe, now);
        out["tui"] = Value::String(tui.as_str().into());
        out["claude"] = Value::String(claude.as_str().into());
        Ok(out)
    }
}
