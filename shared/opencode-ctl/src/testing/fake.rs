//! Scriptable in-memory `Transport`.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::clock::{Clock, TestClock};
use crate::error::ErrorCode;
use crate::openapi::{validate_params, validate_session_id, Operation};
use crate::transport::{Lifecycle, Params, Transport, TransportError, WaitOutcome};

#[derive(Debug, Clone)]
pub struct RecordedCall {
    pub op: String,
    pub params: Params,
    pub body: Option<Value>,
    pub at_ms: i64,
}

struct Script {
    subset: Vec<(String, String)>,
    responses: Vec<Result<Value, TransportError>>,
    next: usize,
}

struct State {
    scripts: HashMap<String, Vec<Script>>,
    waits: VecDeque<(Result<WaitOutcome, TransportError>, u64)>,
    lifecycle: HashMap<Lifecycle, Result<String, TransportError>>,
    lifecycle_seq: HashMap<Lifecycle, VecDeque<Result<String, TransportError>>>,
    cli_version: Result<String, TransportError>,
    openapi: Option<Value>,
    calls: Vec<RecordedCall>,
    wait_calls: Vec<(String, u64, i64)>,
    lifecycle_calls: Vec<(Lifecycle, i64)>,
    openapi_fetches: usize,
}

/// Cloneable handle: clones share one script and one call log.
#[derive(Clone)]
pub struct FakeTransport {
    state: Arc<Mutex<State>>,
    clock: Option<TestClock>,
}

fn err(code: ErrorCode, message: impl Into<String>) -> TransportError {
    TransportError {
        code,
        message: message.into(),
        hint: None,
        details: None,
    }
}

impl Default for FakeTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeTransport {
    pub fn new() -> Self {
        let openapi: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/openapi.json"))
                .expect("fixture openapi.json is valid JSON");
        Self {
            state: Arc::new(Mutex::new(State {
                scripts: HashMap::new(),
                waits: VecDeque::new(),
                lifecycle: HashMap::new(),
                lifecycle_seq: HashMap::new(),
                cli_version: Ok("opencode v2.0.32".to_string()),
                openapi: Some(openapi),
                calls: Vec::new(),
                wait_calls: Vec::new(),
                lifecycle_calls: Vec::new(),
                openapi_fetches: 0,
            })),
            clock: None,
        }
    }

    pub fn with_clock(mut self, clock: TestClock) -> Self {
        self.clock = Some(clock);
        self
    }

    fn now(&self) -> i64 {
        self.clock.as_ref().map_or(0, Clock::now_ms)
    }

    pub fn on(&self, op_id: &str, responses: Vec<Result<Value, TransportError>>) {
        self.on_matching(op_id, &[], responses);
    }

    /// Consumed in order, the last repeats; the script with the largest matching subset wins.
    pub fn on_matching(
        &self,
        op_id: &str,
        params_subset: &[(&str, &str)],
        responses: Vec<Result<Value, TransportError>>,
    ) {
        let subset = params_subset
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.state
            .lock()
            .unwrap()
            .scripts
            .entry(op_id.to_string())
            .or_default()
            .push(Script {
                subset,
                responses,
                next: 0,
            });
    }

    pub fn on_wait(&self, script: Vec<(Result<WaitOutcome, TransportError>, u64)>) {
        self.state.lock().unwrap().waits = script.into();
    }

    pub fn on_lifecycle(&self, verb: Lifecycle, result: Result<String, TransportError>) {
        self.state.lock().unwrap().lifecycle.insert(verb, result);
    }

    /// Answers for `verb` in order, one per call; once used up, `on_lifecycle`'s answer (or the default).
    pub fn on_lifecycle_seq(&self, verb: Lifecycle, results: Vec<Result<String, TransportError>>) {
        self.state
            .lock()
            .unwrap()
            .lifecycle_seq
            .insert(verb, results.into());
    }

    pub fn on_cli_version(&self, result: Result<String, TransportError>) {
        self.state.lock().unwrap().cli_version = result;
    }

    pub fn set_openapi(&self, doc: Option<Value>) {
        self.state.lock().unwrap().openapi = doc;
    }

    pub fn calls(&self) -> Vec<RecordedCall> {
        self.state.lock().unwrap().calls.clone()
    }

    pub fn wait_calls(&self) -> Vec<(String, u64, i64)> {
        self.state.lock().unwrap().wait_calls.clone()
    }

    pub fn lifecycle_calls(&self) -> Vec<(Lifecycle, i64)> {
        self.state.lock().unwrap().lifecycle_calls.clone()
    }

    pub fn openapi_fetches(&self) -> usize {
        self.state.lock().unwrap().openapi_fetches
    }
}

impl Transport for FakeTransport {
    async fn call(
        &self,
        op: &Operation,
        params: &Params,
        body: Option<&Value>,
        _deadline: Duration,
    ) -> Result<Value, TransportError> {
        // The real transport's own parameter checks (`transport/cli.rs`): an undeclared param is rejected there, so here.
        let from_tool = |e: crate::error::ToolError| TransportError {
            code: e.code,
            message: e.message,
            hint: e.hint,
            details: e.details,
        };
        validate_params(op, params).map_err(from_tool)?;
        for (name, value) in params {
            if name == "sessionID" {
                validate_session_id(value).map_err(from_tool)?;
            }
        }
        let at_ms = self.now();
        let mut st = self.state.lock().unwrap();
        st.calls.push(RecordedCall {
            op: op.id.clone(),
            params: params.clone(),
            body: body.cloned(),
            at_ms,
        });
        let best = st.scripts.get_mut(&op.id).and_then(|list| {
            list.iter_mut()
                .filter(|s| s.subset.iter().all(|kv| params.contains(kv)))
                .max_by_key(|s| s.subset.len())
        });
        match best {
            Some(s) if !s.responses.is_empty() => {
                let i = s.next.min(s.responses.len() - 1);
                s.next += 1;
                s.responses[i].clone()
            }
            _ => Err(err(ErrorCode::CliError, format!("unscripted {}", op.id))),
        }
    }

    async fn lifecycle(&self, verb: Lifecycle) -> Result<String, TransportError> {
        let at_ms = self.now();
        let mut st = self.state.lock().unwrap();
        st.lifecycle_calls.push((verb, at_ms));
        if let Some(r) = st
            .lifecycle_seq
            .get_mut(&verb)
            .and_then(VecDeque::pop_front)
        {
            return r;
        }
        match st.lifecycle.get(&verb) {
            Some(r) => r.clone(),
            None => Ok(match verb {
                Lifecycle::Status | Lifecycle::Start => "http://127.0.0.1:49374".to_string(),
                Lifecycle::Stop | Lifecycle::Restart => String::new(),
            }),
        }
    }

    async fn wait_idle(
        &self,
        session_id: &str,
        deadline: Duration,
    ) -> Result<WaitOutcome, TransportError> {
        let clock = self
            .clock
            .clone()
            .expect("FakeTransport::wait_idle needs with_clock(TestClock)");
        let at_ms = clock.now_ms();
        let (result, virtual_ms) = {
            let mut st = self.state.lock().unwrap();
            st.wait_calls
                .push((session_id.to_string(), deadline.as_millis() as u64, at_ms));
            let item = if st.waits.len() > 1 {
                st.waits.pop_front()
            } else {
                st.waits.front().cloned()
            };
            item.unwrap_or((Err(err(ErrorCode::CliError, "unscripted wait_idle")), 0))
        };
        let slept = virtual_ms.min(deadline.as_millis() as u64);
        clock.sleep(Duration::from_millis(slept)).await;
        result
    }

    async fn fetch_openapi(&self) -> Result<Value, TransportError> {
        let mut st = self.state.lock().unwrap();
        st.openapi_fetches += 1;
        st.openapi
            .clone()
            .ok_or_else(|| err(ErrorCode::ServiceDown, "service not reachable"))
    }

    async fn cli_version(&self) -> Result<String, TransportError> {
        self.state.lock().unwrap().cli_version.clone()
    }
}
