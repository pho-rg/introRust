use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub username: String,
    pub content: String,
    pub timestamp: u64,
    pub message_type: MessageType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Text,
    UserJoined,
    UserLeft,
    System,
}

#[derive(Debug)]
pub struct Client {
    pub id: String,
    pub username: String,
    pub addr: SocketAddr,
}

pub struct ServerState {
    pub clients: RwLock<HashMap<String, Client>>,
    pub broadcast_tx: broadcast::Sender<ChatMessage>,
}

impl ServerState {
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        Self {
            clients: RwLock::new(HashMap::new()),
            broadcast_tx,
        }
    }

    pub async fn add_client(&self, client: Client) {
        let mut clients = self.clients.write().await;
        clients.insert(client.id.clone(), client);
    }

    pub async fn remove_client(&self, client_id: &str) -> Option<Client> {
        let mut clients = self.clients.write().await;
        clients.remove(client_id)
    }

    pub async fn get_client_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }

    pub async fn broadcast_message(&self, message: ChatMessage) {
        if let Err(e) = self.broadcast_tx.send(message) {
            eprintln!("Erreur lors de la diffusion du message: {}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(&addr).await?;
    println!("Serveur WebSocket démarré sur ws://{}", addr);

    let state = Arc::new(ServerState::new());

    while let Ok((stream, addr)) = listener.accept().await {
        let state_clone = Arc::clone(&state);
        tokio::spawn(handle_connection(stream, addr, state_clone));
    }

    Ok(())
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<ServerState>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Nouvelle connexion depuis: {}", addr);

    // Effectuer le handshake WebSocket
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Générer un ID unique pour le client
    let client_id = Uuid::new_v4().to_string();
    let mut username = format!("User_{}", &client_id[..8]);

    // Créer un récepteur pour les messages broadcast
    let mut broadcast_rx = state.broadcast_tx.subscribe();

    // Tâche pour recevoir les messages du client
    let state_for_receiver = Arc::clone(&state);
    let client_id_for_receiver = client_id.clone();
    let username_for_receiver = username.clone();
    
    let receive_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                        // Gérer différents types de messages
                        if let Some(msg_type) = parsed.get("type").and_then(|v| v.as_str()) {
                            match msg_type {
                                "join" => {
                                    if let Some(new_username) = parsed.get("username").and_then(|v| v.as_str()) {
                                        username = new_username.to_string();
                                        
                                        let client = Client {
                                            id: client_id_for_receiver.clone(),
                                            username: username.clone(),
                                            addr,
                                        };
                                        
                                        state_for_receiver.add_client(client).await;
                                        
                                        let join_message = ChatMessage {
                                            id: Uuid::new_v4().to_string(),
                                            username: "Système".to_string(),
                                            content: format!("{} a rejoint le chat", username),
                                            timestamp: std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_secs(),
                                            message_type: MessageType::UserJoined,
                                        };
                                        
                                        state_for_receiver.broadcast_message(join_message).await;
                                        
                                        println!("Client {} ({}) a rejoint le chat", username, client_id_for_receiver);
                                    }
                                }
                                "message" => {
                                    if let Some(content) = parsed.get("content").and_then(|v| v.as_str()) {
                                        let chat_message = ChatMessage {
                                            id: Uuid::new_v4().to_string(),
                                            username: username_for_receiver.clone(),
                                            content: content.to_string(),
                                            timestamp: std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_secs(),
                                            message_type: MessageType::Text,
                                        };
                                        
                                        state_for_receiver.broadcast_message(chat_message).await;
                                    }
                                }
                                _ => {
                                    println!("Type de message non reconnu: {}", msg_type);
                                }
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("Client {} a fermé la connexion", client_id_for_receiver);
                    break;
                }
                Err(e) => {
                    eprintln!("Erreur WebSocket: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Tâche pour diffuser les messages aux clients
    let broadcast_task = tokio::spawn(async move {
        while let Ok(message) = broadcast_rx.recv().await {
            let json_message = serde_json::to_string(&message).unwrap();
            if let Err(e) = ws_sender.send(Message::Text(json_message)).await {
                eprintln!("Erreur lors de l'envoi du message: {}", e);
                break;
            }
        }
    });

    // Attendre qu'une des tâches se termine
    tokio::select! {
        _ = receive_task => {},
        _ = broadcast_task => {},
    }

    // Nettoyer le client déconnecté
    if let Some(client) = state.remove_client(&client_id).await {
        let leave_message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            username: "Système".to_string(),
            content: format!("{} a quitté le chat", client.username),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            message_type: MessageType::UserLeft,
        };
        
        state.broadcast_message(leave_message).await;
        println!("Client {} déconnecté", client.username);
    }

    Ok(())
}