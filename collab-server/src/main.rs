use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::Method,
    response::IntoResponse,
    routing::get,
};
use futures_util::{SinkExt, StreamExt, future::join_all, stream::SplitSink};
use serde::Serialize;
use tokio::{net::TcpListener, sync::Mutex};
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct AppState {
    active_rooms: Arc<Mutex<HashMap<String, Arc<Mutex<Room>>>>>,
}

struct Room {
    id: String,
    room_state: Arc<Mutex<Document>>,
    participants: Vec<Participant>,
}

struct Participant {
    id: String,
    sender: Arc<Mutex<SplitSink<WebSocket, Message>>>,
}

struct Document {
    content: String,
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!("Server running on http://127.0.0.1:8080");

    let state = AppState {
        active_rooms: Arc::new(Mutex::new(HashMap::new())),
    };

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/create-room", get(create_room))
        .route("/ws/{room_id}", get(join_room))
        .with_state(state)
        .layer(cors);

    axum::serve(listener, app).await.unwrap();
}

#[derive(Serialize)]
struct CreateRoomResponse {
    room_id: String,
}

async fn create_room(State(state): State<AppState>) -> Json<CreateRoomResponse> {
    let mut rooms = state.active_rooms.lock().await;
    let room_id = format!("room_{}", rooms.len() + 1);
    let room = Room {
        id: room_id.clone(),
        participants: Vec::new(),
        room_state: Arc::new(Mutex::new(Document {
            content: String::new(),
        })),
    };

    rooms.insert(room_id.clone(), Arc::new(Mutex::new(room)));

    println!("Room created: {}", room_id);

    Json(CreateRoomResponse { room_id })
}

async fn join_room(
    ws: WebSocketUpgrade,
    Path(room_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, room_id, state))
}

async fn handle_socket(socket: WebSocket, room_id: String, state: AppState) {
    println!("Socket upgraded");
    let (sender, mut receiver) = socket.split();

    let sender = Arc::new(Mutex::new(sender));
    // Register participant
    let participant_id;
    {
        let mut rooms = state.active_rooms.lock().await;
        if let Some(room) = rooms.get_mut(&room_id) {
            let mut room = room.lock().await;
            participant_id = format!("participant_{}", room.participants.len() + 1);
            room.participants.push(Participant {
                id: participant_id.clone(),
                sender: sender.clone(),
            });
        } else {
            println!("Room {} not found", room_id);
            return;
        }
    }

    // Clone state & room_id for async task
    let state_for_task = state.clone();
    let room_id_for_task = room_id.clone();

    println!("Participant {} joined room {}", participant_id, room_id);

    println!(
        "Spawning task to handle messages for participant {}",
        participant_id
    );

    tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            match message {
                Message::Text(text) => {
                    // Update document
                    let rooms = state_for_task.active_rooms.lock().await;
                    if let Some(room) = rooms.get(&room_id_for_task) {
                        let room = room.lock().await;
                        let mut doc = room.room_state.lock().await;
                        doc.content = text.clone().to_string();
                        println!("Document updated: {}", doc.content);

                        // Broadcast to all participants
                        for participant in &room.participants {
                            let mut s = participant.sender.lock().await;
                            s.send(Message::Text(text.clone())).await.unwrap();
                        }

                        let futures: Vec<_> = room
                            .participants
                            .iter()
                            .map(|participant| {
                                let text = text.clone();
                                let sender = participant.sender.clone();

                                async move {
                                    let mut s = sender.lock().await;
                                    if let Err(e) = s.send(Message::Text(text)).await {
                                        eprintln!("Failed to send: {:?}", e);
                                    }
                                }
                            })
                            .collect();

                        join_all(futures).await;
                    }
                }
                Message::Close(_) => {
                    println!("Participant {} disconnected", participant_id);
                    let mut rooms = state_for_task.active_rooms.lock().await;
                    if let Some(room) = rooms.get_mut(&room_id_for_task) {
                        let mut room = room.lock().await;
                        room.participants.retain(|p| p.id != participant_id);
                    }
                    break;
                }
                _ => {
                    println!("Received non-text message");
                }
            }
        }
    });
}
