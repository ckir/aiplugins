use opencode_ctl::error::ErrorCode;
use opencode_ctl::openapi::{
    body_required_of, gate, glob_matches, validate_pair_id, validate_params, validate_session_id,
    GateInput, Method, OperationTable, ParamIn, TableSource, GET_DENY_LIST, MUTATION_ALLOW_LIST,
    SNAPSHOT_VERSION,
};
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("fixtures/openapi.json");

fn doc() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

fn snap() -> OperationTable {
    OperationTable::snapshot()
}

fn run(
    live: Option<&OperationTable>,
    snapshot: &OperationTable,
    id: &str,
    allow: bool,
    locked: bool,
) -> Result<TableSource, (ErrorCode, Option<String>, Option<Value>)> {
    gate(GateInput {
        live,
        snapshot,
        id,
        allow_mutation: allow,
        locked,
        server_version: Some("2.0.32"),
    })
    .map(|(_, s)| s)
    .map_err(|e| (e.code, e.hint, e.details))
}

fn code(r: Result<TableSource, (ErrorCode, Option<String>, Option<Value>)>) -> ErrorCode {
    r.unwrap_err().0
}

#[test]
fn snapshot_table_matches_fixture_document() {
    let t = OperationTable::from_openapi(&doc(), TableSource::Live).unwrap();
    let committed: Value = serde_json::from_str(include_str!("../src/openapi.table.json")).unwrap();
    assert_eq!(t.to_json(), committed);
    assert_eq!(committed["version"], SNAPSHOT_VERSION);
}

#[test]
fn snapshot_counts() {
    let t = snap();
    assert_eq!(t.source, TableSource::Snapshot);
    assert_eq!(t.len(), 143);
    assert_eq!(t.iter().filter(|o| o.success_status == 204).count(), 46);
    assert_eq!(t.iter().filter(|o| o.method == Method::Get).count(), 66);
    assert_eq!(t.iter().filter(|o| o.mutating()).count(), 77);
    let w = t.get("experimental.session.wait").unwrap();
    assert_eq!((w.method, w.success_status), (Method::Post, 204));
    let i = t.get("session.interrupt").unwrap();
    assert_eq!(
        (i.method, i.success_status, i.json_response),
        (Method::Post, 200, true)
    );
    assert!(i
        .params
        .iter()
        .any(|p| p.name == "resume" && p.location == ParamIn::Query));
    assert_eq!(
        t.get("session.permission.reply").unwrap().body_required,
        vec!["decision".to_string()]
    );
}

#[test]
fn hatch_allowlist_is_exactly_these_seven() {
    assert_eq!(
        MUTATION_ALLOW_LIST,
        [
            "session.compact",
            "session.switchAgent",
            "session.switchModel",
            "session.view",
            "session.inbox.cancel",
            "session.inbox.update",
            "session.form.cancel"
        ]
    );
}

#[test]
fn hatch_snapshot_only_id_unknown_when_live_present() {
    let mut d = doc();
    for item in d["paths"].as_object_mut().unwrap().values_mut() {
        item.as_object_mut()
            .unwrap()
            .retain(|_, op| op["operationId"] != "session.get");
    }
    let live = OperationTable::from_openapi(&d, TableSource::Live).unwrap();
    assert!(live.get("session.get").is_none());
    let s = snap();
    let (c, hint, _) = run(Some(&live), &s, "session.get", false, false).unwrap_err();
    assert_eq!(c, ErrorCode::UnknownOperation);
    let hint = hint.unwrap();
    assert!(hint.starts_with("not in the live document"), "{hint}");
    assert!(hint.contains("2.0.32"));
    let (op, src) = gate(GateInput {
        live: None,
        snapshot: &s,
        id: "session.get",
        allow_mutation: false,
        locked: false,
        server_version: None,
    })
    .unwrap();
    assert_eq!(
        (op.id.as_str(), src),
        ("session.get", TableSource::Snapshot)
    );
}

#[test]
fn hatch_get_denylist_blocks_every_listed_id() {
    let s = snap();
    for pat in GET_DENY_LIST {
        assert!(
            s.iter()
                .any(|o| o.method == Method::Get && glob_matches(pat, &o.id)),
            "pattern {pat} matches nothing"
        );
    }
    let ten = s
        .iter()
        .filter(|o| glob_matches("server.experimental.persistentPty.*", &o.id))
        .count();
    assert_eq!(ten, 10);
    assert!(s
        .get("persistentPty.connect")
        .is_some_and(|o| glob_matches("persistentPty.*", &o.id)));
    for o in s.iter().filter(|o| o.method == Method::Get) {
        if GET_DENY_LIST.iter().any(|p| glob_matches(p, &o.id)) {
            assert_eq!(
                code(run(None, &s, &o.id, true, false)),
                ErrorCode::BlockedOperation,
                "{}",
                o.id
            );
        }
    }
}

#[test]
fn hatch_every_snapshot_mutation_is_allowlisted_or_blocked() {
    let live = OperationTable::from_openapi(&doc(), TableSource::Live).unwrap();
    let s = snap();
    for o in s.iter().filter(|o| o.mutating()) {
        let listed = MUTATION_ALLOW_LIST.contains(&o.id.as_str());
        let without = run(Some(&live), &s, &o.id, false, false);
        let with = run(Some(&live), &s, &o.id, true, false);
        if listed {
            assert_eq!(code(without), ErrorCode::MutationBlocked, "{}", o.id);
            assert!(with.is_ok(), "{} with flag: {with:?}", o.id);
        } else {
            assert_eq!(code(without), ErrorCode::BlockedOperation, "{}", o.id);
            assert_eq!(code(with), ErrorCode::BlockedOperation, "{}", o.id);
        }
    }
}

#[test]
fn hatch_switch_agent_locked() {
    let live = OperationTable::from_openapi(&doc(), TableSource::Live).unwrap();
    let s = snap();
    assert_eq!(
        code(run(Some(&live), &s, "session.switchAgent", true, true)),
        ErrorCode::PermissionsLocked
    );
    assert!(run(Some(&live), &s, "session.switchModel", true, true).is_ok());
}

#[test]
fn hatch_unknown_id() {
    let s = snap();
    let (c, hint, _) = run(None, &s, "session.gett", false, false).unwrap_err();
    assert_eq!(c, ErrorCode::UnknownOperation);
    let hint = hint.unwrap();
    let list = hint.strip_prefix("nearest: ").unwrap();
    assert_eq!(list.split(", ").count(), 3);
    assert!(list.starts_with("session.get"));
}

#[test]
fn hatch_method_path_form_rejected() {
    let s = snap();
    for id in [
        "GET /session",
        "/api/session",
        "session.get extra",
        "GET\tx",
    ] {
        assert_eq!(
            code(run(None, &s, id, false, false)),
            ErrorCode::InvalidArgument,
            "{id}"
        );
    }
}

#[test]
fn hatch_unknown_to_snapshot_blocked() {
    let mut d = doc();
    d["paths"]["/api/zz-new"] = json!({"get": {"operationId": "zz.new", "parameters": [], "responses": {"200": {"description": "ok", "content": {"application/json": {}}}}}});
    let live = OperationTable::from_openapi(&d, TableSource::Live).unwrap();
    let (c, _, details) = run(Some(&live), &snap(), "zz.new", false, false).unwrap_err();
    assert_eq!(c, ErrorCode::BlockedOperation);
    assert_eq!(details.unwrap()["reason"], "unknown_to_snapshot");
}

#[test]
fn hatch_mutation_never_from_snapshot() {
    assert_eq!(
        code(run(None, &snap(), "session.compact", true, false)),
        ErrorCode::ServiceDown
    );
}

#[test]
fn hatch_non_json_refused() {
    let mut d = doc();
    d["paths"]["/api/vcs/status"]["get"]["responses"]["200"]["content"] =
        json!({"text/event-stream": {}});
    let live = OperationTable::from_openapi(&d, TableSource::Live).unwrap();
    assert!(!live.get("vcs.status").unwrap().json_response);
    assert_eq!(
        code(run(Some(&live), &snap(), "vcs.status", false, false)),
        ErrorCode::NonJsonResponse
    );
}

#[test]
fn hatch_param_name_prefix_and_value_rules() {
    let s = snap();
    let p = |n: &str, v: &str| vec![(n.to_string(), v.to_string())];
    let vcs = s.get("vcs.status").unwrap();
    assert!(validate_params(vcs, &p("location[directory]", "/tmp")).is_ok());
    assert_eq!(
        validate_params(vcs, &p("directory", "/tmp"))
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    let get = s.get("session.get").unwrap();
    assert!(validate_params(get, &p("sessionID", "ses_x")).is_ok());
    for (n, v) in [
        ("sessionID", "a\nb"),
        ("sessionID", "a\0b"),
        ("", "x"),
        ("sessionID", ""),
        ("header", "x"),
        ("-H", "x"),
    ] {
        assert_eq!(
            validate_params(get, &p(n, v)).unwrap_err().code,
            ErrorCode::InvalidArgument,
            "{n:?}={v:?}"
        );
    }
}

#[test]
fn body_required_resolves_ref() {
    let d = doc();
    let find = |id: &str| -> Value {
        d["paths"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|i| i.as_object().unwrap().values())
            .find(|o| o["operationId"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(
        body_required_of(&find("session.form.reply"), &d),
        vec!["answer".to_string()]
    );
    assert_eq!(
        body_required_of(&find("session.permission.reply"), &d),
        vec!["decision".to_string()]
    );
}

#[test]
fn glob_semantics() {
    assert!(glob_matches("fs.*", "fs.read"));
    assert!(!glob_matches("fs.*", "fsx.read"));
    assert!(!glob_matches("fs.*", "fs"));
    assert!(glob_matches("session.log", "session.log"));
    assert!(!glob_matches("session.log", "session.logs"));
    assert!(!glob_matches("fs.*", "FS.read"));
}

#[test]
fn transport_session_id_validated() {
    let ok = format!("ses_{}", "a".repeat(26));
    assert!(validate_session_id(&ok).is_ok());
    assert!(validate_session_id(&format!("ses_{}", "aB3".repeat(8) + "xy")).is_ok());
    for bad in [
        format!("ses_{}", "a".repeat(25)),
        format!("ses_{}", "a".repeat(27)),
        format!("ses_{}-", "a".repeat(25)),
        String::new(),
        "x".repeat(30),
    ] {
        assert_eq!(
            validate_session_id(&bad).unwrap_err().code,
            ErrorCode::SessionNotFound,
            "{bad}"
        );
    }
    assert!(validate_pair_id("0123456789ab").is_ok());
    assert_eq!(
        validate_pair_id("0123456789AB").unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        validate_pair_id("0123456789a").unwrap_err().code,
        ErrorCode::InvalidArgument
    );
}

#[test]
#[ignore]
fn regenerate_snapshot_table() {
    let t = OperationTable::from_openapi(&doc(), TableSource::Live).unwrap();
    let mut s = serde_json::to_string_pretty(&t.to_json()).unwrap();
    s.push('\n');
    std::fs::write(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/openapi.table.json"),
        s,
    )
    .unwrap();
}
