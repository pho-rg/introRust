use chrono::{DateTime, Utc};
use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[derive(Debug)]
struct LogServer {
    log_file_path: String,
    client_count: Arc<Mutex<u32>>,
}

impl LogServer {
    fn new(log_file_path: String) -> Self {
        LogServer {
            log_file_path,
            client_count: Arc::new(Mutex::new(0)),
        }
    }

    async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = std::path::Path::new(&self.log_file_path).parent() {
            fs::create_dir_all(parent).await?;
        }
        self.write_log("SERVER", "Serveur demarre").await?;
        println!("Serveur de journalisation initialise");
        println!("Fichier de logs: {}", self.log_file_path);
        Ok(())
    }

    async fn write_log(&self, client_id: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        let timestamp: DateTime<Utc> = Utc::now();
        let log_entry = format!(
            "[{}] [{}] {}\n",
            timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            client_id,
            message.trim()
        );

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)?;

        file.write_all(log_entry.as_bytes())?;
        file.flush()?;

        Ok(())
    }

    async fn increment_client_count(&self) -> u32 {
        let mut count = self.client_count.lock().await;
        *count += 1;
        *count
    }

    async fn decrement_client_count(&self) -> u32 {
        let mut count = self.client_count.lock().await;
        if *count > 0 {
            *count -= 1;
        }
        *count
    }

    async fn get_client_count(&self) -> u32 {
        let count = self.client_count.lock().await;
        *count
    }

    async fn handle_client(
        &self,
        stream: TcpStream,
        client_addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client_id = format!("CLIENT-{}", client_addr);
        let client_num = self.increment_client_count().await;

        self.write_log(&client_id, &format!("Connexion client #{}", client_num)).await?;

        let (reader, mut writer) = stream.into_split();
        let reader = BufReader::new(reader);
        let mut lines = reader.lines();

        let welcome_msg = format!(
            "Bienvenue sur le serveur de log\nID: {}\nClients connectes: {}\nTapez vos messages (CTRL+C pour quitter)",
            client_id, self.get_client_count().await
        );
        let _ = writer.write_all(welcome_msg.as_bytes()).await;

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() {
                        continue;
                    }

                    match line.trim().to_lowercase().as_str() {
                        "exit" => {
                            let _ = writer.write_all(b"Au revoir\n").await;
                            break;
                        }
                        _ => {
                            self.write_log(&client_id, &line).await?;
                            let _ = writer.write_all(b"Message enregistre\n").await;
                        }
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    self.write_log(&client_id, &format!("Erreur lecture: {}", e)).await?;
                    eprintln!("Erreur lecture client {}: {}", client_addr, e);
                    break;
                }
            }
        }

        let remaining_clients = self.decrement_client_count().await;
        self.write_log(&client_id, &format!("Deconnexion. Clients restants: {}", remaining_clients)).await?;

        println!("Client {} deconnecte. Clients restants: {}", client_addr, remaining_clients);

        Ok(())
    }

    async fn run(&self, bind_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.initialize().await?;

        let listener = TcpListener::bind(bind_addr).await?;
        println!("Serveur en ecoute sur {}", bind_addr);
        println!("Les logs sont enregistres dans: {}", self.log_file_path);
        println!("En attente de connexions clients...\n");

        loop {
            match listener.accept().await {
                Ok((stream, client_addr)) => {
                    println!("Nouvelle connexion de: {}", client_addr);

                    let server_clone = LogServer {
                        log_file_path: self.log_file_path.clone(),
                        client_count: Arc::clone(&self.client_count),
                    };

                    tokio::spawn(async move {
                        if let Err(e) = server_clone.handle_client(stream, client_addr).await {
                            eprintln!("Erreur traitement client {}: {}", client_addr, e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Erreur acceptation connexion: {}", e);
                    self.write_log("SERVER", &format!("Erreur acceptation connexion: {}", e)).await?;
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("---");
    println!("SERVEUR DE LOG");
    println!("---");

    let bind_addr = "127.0.0.1:8080";
    let log_file_path = "logs/server.log".to_string();

    let server = LogServer::new(log_file_path);

    let server_task = tokio::spawn(async move {
        if let Err(e) = server.run(bind_addr).await {
            eprintln!("Erreur serveur: {}", e);
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nSignal d'arret recu (Ctrl+C)");
            println!("Arret du serveur en cours...");
        }
        _ = server_task => {
            println!("Serveur termine");
        }
    }

    println!("Serveur de journalisation arrete");
    Ok(())
}
