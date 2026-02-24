mod db;
mod sanitizer;
mod ws;

use actix_web::{web, App, HttpServer, HttpResponse, Responder, HttpRequest, cookie::Cookie};
use actix_multipart::{Multipart, Field};
use actix_files::Files;
use futures_util::StreamExt as _;
use std::io::Write;
use uuid::Uuid;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use sqlx::Row; // Needed for dynamic SQL querying

struct AppState {
    db: sqlx::SqlitePool,
    chat_server: Arc<ws::ChatServer>,
}

async fn upload_file(mut field: Field, folder: &str) -> Option<String> {
    let content_disposition = field.content_disposition().clone();
    let filename = content_disposition.get_filename().unwrap_or("");
    if filename.is_empty() {
        while let Some(_) = field.next().await {}
        return None;
    }
    let dir = format!("./uploads/{}", folder);
    let _ = std::fs::create_dir_all(&dir);
    let ext = filename.split('.').last().unwrap_or("png");
    let unique_name = format!("{}.{}", Uuid::new_v4(), ext);
    let filepath = format!("{}/{}", dir, unique_name);
    let public_path = format!("/uploads/{}/{}", folder, unique_name);

    if let Ok(mut f) = std::fs::File::create(&filepath) {
        while let Some(chunk) = field.next().await {
            if let Ok(data) = chunk {
                let _ = f.write_all(&data);
            }
        }
        Some(public_path)
    } else { None }
}

async fn register_user(state: web::Data<AppState>, mut payload: Multipart) -> impl Responder {
    let mut name = String::new();
    let mut pfp_path = String::from("/static/default.png");

    while let Some(item) = payload.next().await {
        let mut field = match item { Ok(f) => f, Err(_) => continue };
        let field_name = field.content_disposition().get_name().unwrap_or("").to_string();
        if field_name == "name" {
            while let Some(chunk) = field.next().await {
                if let Ok(data) = chunk { name.push_str(&String::from_utf8_lossy(&data)); }
            }
        } else if field_name == "pfp" {
            if let Some(path) = upload_file(field, "pfps").await { pfp_path = path; }
        } else {
            while let Some(_) = field.next().await {}
        }
    }

    let user_id = Uuid::new_v4().to_string();
    let _ = sqlx::query("INSERT INTO users (id, name, pfp_path) VALUES (?, ?, ?)")
        .bind(&user_id).bind(&name).bind(&pfp_path)
        .execute(&state.db).await;

    let cookie = Cookie::build("user_id", user_id).path("/").http_only(false).finish();
    HttpResponse::Found().cookie(cookie).append_header(("Location", "/static/index.html")).finish()
}

async fn send_message(state: web::Data<AppState>, req: HttpRequest, mut payload: Multipart) -> impl Responder {
    let user_id = match req.cookie("user_id") {
        Some(c) => c.value().to_string(),
        None => return HttpResponse::Unauthorized().body("Missing User Cookie"),
    };
    
    let mut content = String::new();
    let mut attachment_path = None;

    while let Some(item) = payload.next().await {
        let mut field = match item { Ok(f) => f, Err(_) => continue };
        let field_name = field.content_disposition().get_name().unwrap_or("").to_string();
        if field_name == "content" {
            while let Some(chunk) = field.next().await {
                if let Ok(data) = chunk { content.push_str(&String::from_utf8_lossy(&data)); }
            }
        } else if field_name == "attachment" {
            attachment_path = upload_file(field, "attachments").await;
        } else {
            while let Some(_) = field.next().await {}
        }
    }

    let safe_content = sanitizer::sanitize_chat_message(&content);
    let msg_id = Uuid::new_v4().to_string();

    let _ = sqlx::query("INSERT INTO messages (id, user_id, content, attachment_path) VALUES (?, ?, ?, ?)")
        .bind(&msg_id).bind(&user_id).bind(&safe_content).bind(&attachment_path)
        .execute(&state.db).await;

    // Fetch user details for the rich broadcast
    if let Ok(row) = sqlx::query("SELECT name, pfp_path FROM users WHERE id = ?").bind(&user_id).fetch_one(&state.db).await {
        let name: String = row.get("name");
        let pfp: String = row.get("pfp_path");

        let msg_json = serde_json::json!({
            "type": "new_msg",
            "name": name,
            "pfp": pfp,
            "content": safe_content,
            "attachment": attachment_path
        });
        state.chat_server.broadcast(&msg_json.to_string(), None);
    }

    HttpResponse::Ok().body("Sent")
}

async fn ws_route(req: HttpRequest, stream: web::Payload, state: web::Data<AppState>) -> Result<HttpResponse, actix_web::Error> {
    let user_id = match req.cookie("user_id") {
        Some(c) => c.value().to_string(),
        None => return Err(actix_web::error::ErrorUnauthorized("Missing cookie")),
    };

    // Query DB just once on connection to populate the in-memory Rust variable
    let row = sqlx::query("SELECT name, pfp_path FROM users WHERE id = ?")
        .bind(&user_id).fetch_optional(&state.db).await.unwrap_or(None);

    let (name, pfp) = match row {
        Some(r) => {
            let n: String = r.get("name");
            let p: String = r.get("pfp_path");
            (n, p)
        },
        None => ("Anonymous".to_string(), "/static/default.png".to_string()),
    };

    let user_info = ws::UserInfo { id: user_id, name, pfp };
    actix_web_actors::ws::start(ws::WsSession { user: user_info, server: state.chat_server.clone() }, &req, stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let db = db::init_db().await;
    let chat_server = Arc::new(ws::ChatServer { 
        sessions: Arc::new(Mutex::new(HashMap::new())),
        online_users: Arc::new(Mutex::new(HashMap::new())), // Initialize the memory variable
    });

    let app_state = web::Data::new(AppState { db, chat_server });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(Files::new("/static", "./static").index_file("index.html"))
            .service(Files::new("/uploads", "./uploads"))
            .route("/register", web::post().to(register_user))
            .route("/message", web::post().to(send_message))
            .route("/ws", web::get().to(ws_route))
            .route("/", web::get().to(|_: HttpRequest| async {
                HttpResponse::Found().append_header(("Location", "/static/index.html")).finish()
            }))
    }).bind(("0.0.0.0", 8080))?.run().await
}