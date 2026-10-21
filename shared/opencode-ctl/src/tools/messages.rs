//! `session_prompt` and `session_messages`.

use serde_json::{json, Value};

use crate::clock::Clock;
use crate::compact::{compact_message, ToolsMode};
use crate::engine::Engine;
use crate::error::{ErrorCode, ToolError, ToolResult};
use crate::transport::Transport;

use super::{SessionMessagesArgs, SessionPromptArgs};

fn invalid(msg: String) -> ToolError {
    ToolError::new(ErrorCode::InvalidArgument, msg)
}

impl<T: Transport, C: Clock> Engine<T, C> {
    pub async fn session_prompt(&self, a: SessionPromptArgs) -> ToolResult {
        let id = self.resolve_session(a.session_id.as_deref())?;
        self.guard_created(&id)?;
        let delivery = a.delivery.as_deref().unwrap_or("steer");
        if delivery != "steer" && delivery != "queue" {
            return Err(invalid(format!(
                "delivery '{delivery}' is not steer or queue"
            )));
        }
        let body = json!({"text": a.text, "delivery": delivery});
        if body.to_string().len() > self.settings.body_max_bytes {
            return Err(ToolError::new(
                ErrorCode::BodyTooLarge,
                format!(
                    "the prompt body exceeds body_max_bytes ({})",
                    self.settings.body_max_bytes
                ),
            )
            .hint("send a shorter prompt, or put the detail in a file the worker can read"));
        }
        let (data, _, _) = self
            .call_json(
                "session.prompt",
                vec![("sessionID".to_string(), id)],
                Some(body),
            )
            .await?;
        Ok(json!({
            "message_id": data.get("id").cloned().unwrap_or(Value::Null),
            "delivery": delivery,
            "hint": "call session_wait with after=<message_id> on EVERY slice",
        }))
    }

    pub async fn session_messages(&self, a: SessionMessagesArgs) -> ToolResult {
        let id = self.resolve_session(a.session_id.as_deref())?;
        let tools = match a.tools.as_deref().unwrap_or("summary") {
            "none" => ToolsMode::None,
            "summary" => ToolsMode::Summary,
            "full" => ToolsMode::Full,
            other => {
                return Err(invalid(format!(
                    "tools '{other}' is not none, summary or full"
                )))
            }
        };
        let order = match a.order.as_deref().unwrap_or("desc") {
            o @ ("asc" | "desc") => o.to_string(),
            other => return Err(invalid(format!("order '{other}' is not asc or desc"))),
        };
        let mut params = vec![
            ("sessionID".to_string(), id),
            (
                "limit".to_string(),
                a.limit.unwrap_or(20).clamp(1, 200).to_string(),
            ),
        ];
        match a.cursor {
            // The cursor carries its own order and the route rejects an `order` beside it.
            Some(c) => params.push(("cursor".into(), c)),
            None => params.push(("order".into(), order)),
        }
        if let Some(t) = a.r#type {
            params.push(("type".into(), t));
        }
        let (data, cursor, _) = self.call_json("session.message.list", params, None).await?;
        let reasoning = a.reasoning.unwrap_or(false);
        let messages: Vec<Value> = data
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|r| {
                        serde_json::to_value(compact_message(r, reasoning, tools))
                            .unwrap_or(Value::Null)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let next = cursor
            .as_ref()
            .and_then(|c| c.get("next"))
            .cloned()
            .unwrap_or(Value::Null);
        // `truncated` becomes true (with `original_bytes`) when the final cap cuts the list.
        Ok(json!({"messages": messages, "next_cursor": next, "truncated": false}))
    }
}
