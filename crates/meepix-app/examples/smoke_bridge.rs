//! Test-only loopback bridge: browser interaction tests exercise the real Rust App.
//! No credentials loaded. Not compiled into the desktop application.
use meepix_app::App;
use serde_json::{json, Value};
use tiny_http::{Header, Response, Server};
fn main() {
    let storage = std::env::args()
        .nth(1)
        .expect("temporary storage directory required");
    let app = App::new(storage.into(), None).unwrap();
    let server = Server::http("127.0.0.1:1421").unwrap();
    println!("MeePix test bridge ready");
    for mut request in server.incoming_requests() {
        let origin = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Origin"))
            .map(|h| h.value.as_str());
        if origin != Some("http://127.0.0.1:1420") {
            let _ = request.respond(Response::from_string("Forbidden").with_status_code(403));
            continue;
        }
        let mut response = Response::from_string("");
        if request.method().as_str() != "OPTIONS" {
            if request.method().as_str() != "POST"
                || request.url() != "/invoke"
                || request.body_length().is_some_and(|l| l > 4 * 1024 * 1024)
            {
                let _ =
                    request.respond(Response::from_string("Invalid request").with_status_code(400));
                continue;
            }
            let body: Result<Value, _> =
                serde_json::from_reader(request.as_reader().take(4 * 1024 * 1024));
            let result = body
                .map_err(|_| "Invalid JSON".to_string())
                .and_then(|v| dispatch(&app, &v));
            response = match result {
                Ok(value) => Response::from_string(json!({"ok":value}).to_string()),
                Err(e) => Response::from_string(json!({"error":e}).to_string()),
            };
        }
        for (name, value) in [
            ("Access-Control-Allow-Origin", "http://127.0.0.1:1420"),
            ("Access-Control-Allow-Headers", "Content-Type"),
            ("Access-Control-Allow-Methods", "POST, OPTIONS"),
            ("Content-Type", "application/json"),
        ] {
            response.add_header(Header::from_bytes(name, value).unwrap());
        }
        let _ = request.respond(response);
    }
}
use std::io::Read;
fn dispatch(app: &App, v: &Value) -> Result<Value, String> {
    let args = &v["args"];
    let name = || {
        args["name"]
            .as_str()
            .ok_or_else(|| "Missing name".to_string())
    };
    let result = match v["cmd"].as_str().unwrap_or("") {
        "document_snapshot" => serde_json::to_value(app.snapshot()?),
        "document_create" => serde_json::to_value(app.create(
            args["width"].as_u64().ok_or("Missing width")? as u32,
            args["height"].as_u64().ok_or("Missing height")? as u32,
        )?),
        "document_command" => serde_json::to_value(
            app.command(
                args["expectedRevision"]
                    .as_u64()
                    .ok_or("Missing revision")?,
                serde_json::from_value(args["command"].clone()).map_err(|_| "Invalid command")?,
            )?,
        ),
        "frame_snapshot" => {
            serde_json::to_value(app.frame(args["frameId"].as_str().ok_or("Missing frame")?)?)
        }
        "project_save" => serde_json::to_value(app.save(name()?)?),
        "project_load" => serde_json::to_value(app.load(name()?)?),
        "project_export" => serde_json::to_value(app.export(name()?)?),
        "easel_models" => serde_json::to_value(app.models()?),
        _ => return Err("Not supported by test bridge".into()),
    };
    result.map_err(|_| "Serialization failed".into())
}
