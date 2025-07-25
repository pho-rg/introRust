use std::io::{self, Write};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use clap::Parser;

#[derive(Parser)]
#[command(name = "WebSocket Client")]
#[command(about = "Un client WebSocket simple pour le chat")]
struct Args {
    /// Adresse du serveur WebSocket
    #[arg(short, long, default_value = "ws://127.0.0.1:8080")]
    url: String,
    
    /// Nom d'utilisateur
    #[arg(short, long, default_value = "Anonymous")]
    username: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    println!("Connexion au serveur WebSocket: {}", args.url);
    
    // Se connecter au serveur WebSocket
    let (ws_stream, _) = connect_async(&args.url).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    
    println!("Connexion établie! Tapez vos messages (tapez '/quit' pour quitter)");
    
    // Envoyer le message de connexion
    let join_message = json!({
        "type": "join",
        "username": args.username
    });
    
    ws_sender.send(Message::Text(join_message.to_string())).await?;
    
    // Tâche pour lire les messages du serveur
    let receive_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                        let username = parsed.get("username").and_then(|v| v.as_str()).unwrap_or("Inconnu");
                        let content = parsed.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        let timestamp = parsed.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                        
                        // Formater l'horodatage
                        let datetime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(timestamp);
                        let formatted_time = format!("{:?}", datetime); // Simplification pour l'exemple
                        
                        println!("\r[{}] {}: {}", formatted_time, username, content);
                        print!("> ");
                        io::stdout().flush().unwrap();
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("\nConnexion fermée par le serveur");
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
    
    // Tâche pour lire l'entrée utilisateur
    let send_task = tokio::spawn(async move {
        let stdin = io::stdin();
        let mut input = String::new();
        
        loop {
            print!("> ");
            io::stdout().flush().unwrap();
            
            input.clear();
            if stdin.read_line(&mut input).is_err() {
                break;
            }
            
            let message = input.trim();
            
            if message == "/quit" {
                println!("Déconnexion...");
                break;
            }
            
            if !message.is_empty() {
                let chat_message = json!({
                    "type": "message",
                    "content": message
                });
                
                if let Err(e) = ws_sender.send(Message::Text(chat_message.to_string())).await {
                    eprintln!("Erreur lors de l'envoi: {}", e);
                    break;
                }
            }
        }
    });
    
    // Attendre qu'une des tâches se termine
    tokio::select! {
        _ = receive_task => {},
        _ = send_task => {},
    }
    
    println!("Client fermé");
    Ok(())
}