use actix::{Actor, StreamHandler, AsyncContext, Handler, Message};
use actix_web_actors::ws;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
    pub pfp: String,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct WsMessage(pub String);

pub struct ChatServer {
    pub sessions: Arc<Mutex<HashMap<String, actix::Addr<WsSession>>>>,
    // IN-MEMORY RUST VARIABLE for online users (No DB)
    pub online_users: Arc<Mutex<HashMap<String, UserInfo>>>, 
}

impl ChatServer {
    pub fn broadcast(&self, msg: &str, skip_id: Option<&str>) {
        let sessions = self.sessions.lock().unwrap();
        for (id, addr) in sessions.iter() {
            if Some(id.as_str()) != skip_id {
                addr.do_send(WsMessage(msg.to_string()));
            }
        }
    }

    // Broadcasts the full list of online users to everyone
    pub fn broadcast_online_users(&self) {
        let users_guard = self.online_users.lock().unwrap();
        let users_list: Vec<&UserInfo> = users_guard.values().collect();
        
        if let Ok(json) = serde_json::to_string(&serde_json::json!({
            "type": "online_users",
            "users": users_list
        })) {
            self.broadcast(&json, None);
        }
    }
}

pub struct WsSession {
    pub user: UserInfo,
    pub server: Arc<ChatServer>,
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Add user to the in-memory active lists
        self.server.sessions.lock().unwrap().insert(self.user.id.clone(), ctx.address());
        self.server.online_users.lock().unwrap().insert(self.user.id.clone(), self.user.clone());
        
        // Notify everyone that the list has updated
        self.server.broadcast_online_users();
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        // Remove user from active lists
        self.server.sessions.lock().unwrap().remove(&self.user.id);
        self.server.online_users.lock().unwrap().remove(&self.user.id);
        
        // Notify everyone that someone left
        self.server.broadcast_online_users();
    }
}

impl Handler<WsMessage> for WsSession {
    type Result = ();
    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, _ctx: &mut Self::Context) {
        if let Ok(ws::Message::Text(text)) = msg {
            self.server.broadcast(&text, Some(&self.user.id));
        }
    }
}