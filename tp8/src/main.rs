
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use bincode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    // Messages client -> serveur
    Connect { username: String },
    JoinRoom { room: String },
    SendMessage { room: String, content: String },
    ListRooms,
    ListUsers { room: String },
    Disconnect,
    
    // Messages serveur -> client
    ConnectAck { success: bool, message: String },
    JoinRoomAck { success: bool, room: String, message: String },
    MessageBroadcast { room: String, username: String, content: String, timestamp: u64 },
    RoomList { rooms: Vec<String> },
    UserList { room: String, users: Vec<String> },
    Error { message: String },
    UserJoined { room: String, username: String },
    UserLeft { room: String, username: String },
}

/// Structure principale d'un message du protocole
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub message_type: MessageType,
    pub timestamp: u64,
}

impl ProtocolMessage {
    pub fn new(message_type: MessageType) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            message_type,
            timestamp,
        }
    }
    
    /// Sérialise le message en bytes
    pub fn serialize(&self) -> Result<Vec<u8>, bincode::Error> {
        let data = bincode::serialize(self)?;
        let len = data.len() as u32;
        let mut result = len.to_be_bytes().to_vec();
        result.extend(data);
        Ok(result)
    }
    
    /// Désérialise un message depuis un stream
    pub fn deserialize_from_stream(stream: &mut TcpStream) -> Result<Self, Box<dyn std::error::Error>> {
        let mut reader = BufReader::new(stream);
        
        // Lire la taille du message (4 bytes)
        let mut len_bytes = [0u8; 4];
        std::io::Read::read_exact(&mut reader, &mut len_bytes)?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        
        // Lire le message
        let mut buffer = vec![0u8; len];
        std::io::Read::read_exact(&mut reader, &mut buffer)?;
        
        let message: ProtocolMessage = bincode::deserialize(&buffer)?;
        Ok(message)
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub username: String,
    pub current_room: Option<String>,
}

pub struct ChatServer {
    users: Arc<Mutex<HashMap<String, User>>>,
    rooms: Arc<Mutex<HashMap<String, Vec<String>>>>, // room -> list of usernames
    connections: Arc<Mutex<HashMap<String, TcpStream>>>,
}

impl ChatServer {
    pub fn new() -> Self {
        Self {
            users: Arc::new(Mutex::new(HashMap::new())),
            rooms: Arc::new(Mutex::new(HashMap::new())),
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    pub fn start(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr)?;
        println!("Serveur SimpleChat démarré sur {}", addr);
        
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let server_clone = ChatServer {
                        users: Arc::clone(&self.users),
                        rooms: Arc::clone(&self.rooms),
                        connections: Arc::clone(&self.connections),
                    };
                    
                    thread::spawn(move || {
                        if let Err(e) = server_clone.handle_client(stream) {
                            eprintln!("Erreur client: {}", e);
                        }
                    });
                }
                Err(e) => eprintln!("Erreur connexion: {}", e),
            }
        }
        
        Ok(())
    }
    
    fn handle_client(&self, mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let peer_addr = stream.peer_addr()?;
        println!("Nouvelle connexion: {}", peer_addr);
        
        let mut current_user: Option<String> = None;
        
        loop {
            match ProtocolMessage::deserialize_from_stream(&mut stream) {
                Ok(message) => {
                    match self.process_message(message, &mut current_user, &mut stream) {
                        Ok(should_continue) => {
                            if !should_continue {
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("Erreur traitement message: {}", e);
                            let error_msg = ProtocolMessage::new(
                                MessageType::Error { message: e.to_string() }
                            );
                            let _ = self.send_message(&mut stream, &error_msg);
                        }
                    }
                }
                Err(_) => {
                    // Client déconnecté
                    break;
                }
            }
        }
        
        // Nettoyage à la déconnexion
        if let Some(username) = current_user {
            self.cleanup_user(&username);
            println!("👋 {} s'est déconnecté", username);
        }
        
        Ok(())
    }
    
    fn process_message(
        &self,
        message: ProtocolMessage,
        current_user: &mut Option<String>,
        stream: &mut TcpStream,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        match message.message_type {
            MessageType::Connect { username } => {
                self.handle_connect(username, current_user, stream)?;
            }
            
            MessageType::JoinRoom { room } => {
                if let Some(ref user) = current_user {
                    self.handle_join_room(user.clone(), room, stream)?;
                } else {
                    let error = ProtocolMessage::new(
                        MessageType::Error { message: "Non connecté".to_string() }
                    );
                    self.send_message(stream, &error)?;
                }
            }
            
            MessageType::SendMessage { room, content } => {
                if let Some(ref user) = current_user {
                    self.handle_send_message(user.clone(), room, content)?;
                } else {
                    let error = ProtocolMessage::new(
                        MessageType::Error { message: "Non connecté".to_string() }
                    );
                    self.send_message(stream, &error)?;
                }
            }
            
            MessageType::ListRooms => {
                self.handle_list_rooms(stream)?;
            }
            
            MessageType::ListUsers { room } => {
                self.handle_list_users(room, stream)?;
            }
            
            MessageType::Disconnect => {
                return Ok(false); // Arrêter la boucle
            }
            
            _ => {
                let error = ProtocolMessage::new(
                    MessageType::Error { message: "Message non supporté".to_string() }
                );
                self.send_message(stream, &error)?;
            }
        }
        
        Ok(true)
    }
    
    fn handle_connect(
        &self,
        username: String,
        current_user: &mut Option<String>,
        stream: &mut TcpStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut users = self.users.lock().unwrap();
        
        if users.contains_key(&username) {
            let response = ProtocolMessage::new(
                MessageType::ConnectAck {
                    success: false,
                    message: "Nom d'utilisateur déjà utilisé".to_string(),
                }
            );
            self.send_message(stream, &response)?;
        } else {
            users.insert(username.clone(), User {
                username: username.clone(),
                current_room: None,
            });
            
            // Stocker la connexion
            let mut connections = self.connections.lock().unwrap();
            connections.insert(username.clone(), stream.try_clone()?);
            
            *current_user = Some(username.clone());
            
            let response = ProtocolMessage::new(
                MessageType::ConnectAck {
                    success: true,
                    message: format!("Bienvenue, {} !", username),
                }
            );
            self.send_message(stream, &response)?;
            
            println!("{} s'est connecté", username);
        }
        
        Ok(())
    }
    
    fn handle_join_room(
        &self,
        username: String,
        room: String,
        stream: &mut TcpStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut users = self.users.lock().unwrap();
        let mut rooms = self.rooms.lock().unwrap();
        
        // Quitter l'ancien salon si nécessaire
        if let Some(user) = users.get(&username) {
            if let Some(ref old_room) = user.current_room {
                if let Some(room_users) = rooms.get_mut(old_room) {
                    room_users.retain(|u| u != &username);
                    // Notifier les autres utilisateurs
                    self.broadcast_to_room(old_room, MessageType::UserLeft {
                        room: old_room.clone(),
                        username: username.clone(),
                    }, Some(&username))?;
                }
            }
        }
        
        // Rejoindre le nouveau salon
        rooms.entry(room.clone()).or_insert_with(Vec::new).push(username.clone());
        
        if let Some(user) = users.get_mut(&username) {
            user.current_room = Some(room.clone());
        }
        
        let response = ProtocolMessage::new(
            MessageType::JoinRoomAck {
                success: true,
                room: room.clone(),
                message: format!("Vous avez rejoint le salon {}", room),
            }
        );
        self.send_message(stream, &response)?;
        
        // Notifier les autres utilisateurs
        self.broadcast_to_room(&room, MessageType::UserJoined {
            room: room.clone(),
            username: username.clone(),
        }, Some(&username))?;
        
        println!("{} a rejoint le salon {}", username, room);
        
        Ok(())
    }
    
    fn handle_send_message(
        &self,
        username: String,
        room: String,
        content: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let users = self.users.lock().unwrap();
        
        // Vérifier que l'utilisateur est dans le salon
        if let Some(user) = users.get(&username) {
            if user.current_room.as_ref() == Some(&room) {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                self.broadcast_to_room(&room, MessageType::MessageBroadcast {
                    room: room.clone(),
                    username: username.clone(),
                    content,
                    timestamp,
                }, None)?;
                
                println!("[{}] {}: message envoyé", room, username);
            }
        }
        
        Ok(())
    }
    
    fn handle_list_rooms(&self, stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let rooms = self.rooms.lock().unwrap();
        let room_list: Vec<String> = rooms.keys().cloned().collect();
        
        let response = ProtocolMessage::new(
            MessageType::RoomList { rooms: room_list }
        );
        self.send_message(stream, &response)?;
        
        Ok(())
    }
    
    fn handle_list_users(&self, room: String, stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let rooms = self.rooms.lock().unwrap();
        let users = rooms.get(&room).cloned().unwrap_or_default();
        
        let response = ProtocolMessage::new(
            MessageType::UserList { room, users }
        );
        self.send_message(stream, &response)?;
        
        Ok(())
    }
    
    fn broadcast_to_room(
        &self,
        room: &str,
        message_type: MessageType,
        exclude_user: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let rooms = self.rooms.lock().unwrap();
        let connections = self.connections.lock().unwrap();
        
        if let Some(room_users) = rooms.get(room) {
            let message = ProtocolMessage::new(message_type);
            
            for username in room_users {
                if let Some(exclude) = exclude_user {
                    if username == exclude {
                        continue;
                    }
                }
                
                if let Some(mut stream) = connections.get(username).and_then(|s| s.try_clone().ok()) {
                    let _ = self.send_message(&mut stream, &message);
                }
            }
        }
        
        Ok(())
    }
    
    fn send_message(stream: &mut TcpStream, message: &ProtocolMessage) -> Result<(), Box<dyn std::error::Error>> {
        let data = message.serialize()?;
        stream.write_all(&data)?;
        stream.flush()?;
        Ok(())
    }
    
    fn cleanup_user(&self, username: &str) {
        let mut users = self.users.lock().unwrap();
        let mut rooms = self.rooms.lock().unwrap();
        let mut connections = self.connections.lock().unwrap();
        
        // Retirer l'utilisateur de son salon
        if let Some(user) = users.get(username) {
            if let Some(ref room) = user.current_room {
                if let Some(room_users) = rooms.get_mut(room) {
                    room_users.retain(|u| u != username);
                    // Notifier les autres
                    let _ = self.broadcast_to_room(room, MessageType::UserLeft {
                        room: room.clone(),
                        username: username.to_string(),
                    }, Some(username));
                }
            }
        }
        
        users.remove(username);
        connections.remove(username);
    }
}

pub struct ChatClient {
    stream: Option<TcpStream>,
    username: Option<String>,
    current_room: Option<String>,
}

impl ChatClient {
    pub fn new() -> Self {
        Self {
            stream: None,
            username: None,
            current_room: None,
        }
    }
    
    pub fn connect(&mut self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(addr)?;
        self.stream = Some(stream);
        println!("Connecté au serveur {}", addr);
        Ok(())
    }
    
    pub fn login(&mut self, username: String) -> Result<bool, Box<dyn std::error::Error>> {
        if let Some(ref mut stream) = self.stream {
            let message = ProtocolMessage::new(MessageType::Connect { username: username.clone() });
            self.send_message(stream, &message)?;
            
            let response = ProtocolMessage::deserialize_from_stream(stream)?;
            match response.message_type {
                MessageType::ConnectAck { success, message } => {
                    println!("{}", message);
                    if success {
                        self.username = Some(username);
                    }
                    Ok(success)
                }
                _ => Ok(false),
            }
        } else {
            Err("Non connecté au serveur".into())
        }
    }
    
    pub fn join_room(&mut self, room: String) -> Result<bool, Box<dyn std::error::Error>> {
        if let Some(ref mut stream) = self.stream {
            let message = ProtocolMessage::new(MessageType::JoinRoom { room: room.clone() });
            self.send_message(stream, &message)?;
            
            let response = ProtocolMessage::deserialize_from_stream(stream)?;
            match response.message_type {
                MessageType::JoinRoomAck { success, message, .. } => {
                    println!("{}", message);
                    if success {
                        self.current_room = Some(room);
                    }
                    Ok(success)
                }
                _ => Ok(false),
            }
        } else {
            Err("Non connecté au serveur".into())
        }
    }
    
    pub fn send_message(&mut self, stream: &mut TcpStream, message: &ProtocolMessage) -> Result<(), Box<dyn std::error::Error>> {
        let data = message.serialize()?;
        stream.write_all(&data)?;
        stream.flush()?;
        Ok(())
    }
    
    pub fn send_chat_message(&mut self, content: String) -> Result<(), Box<dyn std::error::Error>> {
        if let (Some(ref mut stream), Some(ref room)) = (&mut self.stream, &self.current_room) {
            let message = ProtocolMessage::new(MessageType::SendMessage {
                room: room.clone(),
                content,
            });
            self.send_message(stream, &message)?;
            Ok(())
        } else {
            Err("Non connecté ou pas dans un salon".into())
        }
    }
    
    pub fn list_rooms(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut stream) = self.stream {
            let message = ProtocolMessage::new(MessageType::ListRooms);
            self.send_message(stream, &message)?;
            
            let response = ProtocolMessage::deserialize_from_stream(stream)?;
            match response.message_type {
                MessageType::RoomList { rooms } => {
                    println!("Salons disponibles:");
                    for room in rooms {
                        println!("  - {}", room);
                    }
                }
                _ => println!("Erreur lors de la récupération des salons"),
            }
            Ok(())
        } else {
            Err("Non connecté au serveur".into())
        }
    }
    
    pub fn start_message_listener(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(stream) = self.stream.take() {
            let mut stream_clone = stream.try_clone()?;
            self.stream = Some(stream);
            
            thread::spawn(move || {
                loop {
                    match ProtocolMessage::deserialize_from_stream(&mut stream_clone) {
                        Ok(message) => {
                            match message.message_type {
                                MessageType::MessageBroadcast { room, username, content, .. } => {
                                    println!("[{}] {}: {}", room, username, content);
                                }
                                MessageType::UserJoined { room, username } => {
                                    println!("{} a rejoint le salon {}", username, room);
                                }
                                MessageType::UserLeft { room, username } => {
                                    println!("{} a quitté le salon {}", username, room);
                                }
                                MessageType::Error { message } => {
                                    println!("Erreur: {}", message);
                                }
                                _ => {}
                            }
                        }
                        Err(_) => {
                            println!("Connexion perdue");
                            break;
                        }
                    }
                }
            });
        }
        
        Ok(())
    }
    
    pub fn disconnect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut stream) = self.stream {
            let message = ProtocolMessage::new(MessageType::Disconnect);
            self.send_message(stream, &message)?;
        }
        self.stream = None;
        self.username = None;
        self.current_room = None;
        println!("Déconnecté");
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        println!("Usage: {} [server|client] [options...]", args[0]);
        println!("  server <addr>           - Démarrer le serveur");
        println!("  client <addr>           - Démarrer le client");
        return Ok(());
    }
    
    match args[1].as_str() {
        "server" => {
            let addr = args.get(2).map(|s| s.as_str()).unwrap_or("127.0.0.1:8080");
            let server = ChatServer::new();
            server.start(addr)?;
        }
        "client" => {
            let addr = args.get(2).map(|s| s.as_str()).unwrap_or("127.0.0.1:8080");
            run_client(addr)?;
        }
        _ => {
            println!("Mode non reconnu. Utilisez 'server' ou 'client'");
        }
    }
    
    Ok(())
}

fn run_client(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ChatClient::new();
    client.connect(addr)?;
    
    // Authentification
    print!("👤 Nom d'utilisateur: ");
    io::stdout().flush()?;
    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    let username = username.trim().to_string();
    
    if !client.login(username)? {
        println!("Échec de l'authentification");
        return Ok(());
    }
    
    // Démarrer l'écoute des messages
    client.start_message_listener()?;
    
    println!("\nCommandes disponibles:");
    println!("  /join <salon>     - Rejoindre un salon");
    println!("  /rooms            - Lister les salons");
    println!("  /quit             - Quitter");
    println!("  <message>         - Envoyer un message dans le salon actuel\n");
    
    // Boucle de commandes
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim();
        
        if line.is_empty() {
            continue;
        }
        
        if line.starts_with('/') {
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            match parts[0] {
                "/join" => {
                    if parts.len() > 1 {
                        let _ = client.join_room(parts[1].to_string());
                    } else {
                        println!("Usage: /join <salon>");
                    }
                }
                "/rooms" => {
                    let _ = client.list_rooms();
                }
                "/quit" => {
                    break;
                }
                _ => {
                    println!("Commande inconnue: {}", parts[0]);
                }
            }
        } else {
            // Message normal
            if let Err(e) = client.send_chat_message(line.to_string()) {
                println!("Erreur envoi message: {}", e);
            }
        }
    }
    
    client.disconnect()?;
    Ok(())
}