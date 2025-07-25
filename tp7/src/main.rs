use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;
use std::io::{Cursor, Result as IoResult};


#[derive(Debug, Clone)]
pub struct DnsHeader {
    pub id: u16,
    pub flags: u16,
    pub qdcount: u16,  // Nombre de questions
    pub ancount: u16,  // Nombre de réponses
    pub nscount: u16,  // Nombre d'enregistrements d'autorité
    pub arcount: u16,  // Nombre d'enregistrements additionnels
}

#[derive(Debug, Clone)]
pub struct DnsQuestion {
    pub qname: String,   // Nom de domaine
    pub qtype: u16,      // Type de requête (A=1, AAAA=28, etc.)
    pub qclass: u16,     // Classe (IN=1 pour Internet)
}

#[derive(Debug, Clone)]
pub struct DnsResourceRecord {
    pub name: String,
    pub rtype: u16,
    pub rclass: u16,
    pub ttl: u32,
    pub rdlength: u16,
    pub rdata: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DnsMessage {
    pub header: DnsHeader,
    pub questions: Vec<DnsQuestion>,
    pub answers: Vec<DnsResourceRecord>,
    pub authority: Vec<DnsResourceRecord>,
    pub additional: Vec<DnsResourceRecord>,
}


impl DnsHeader {
    pub fn new_query(id: u16) -> Self {
        Self {
            id,
            flags: 0x0100, // QR=0 (query), RD=1 (recursion desired)
            qdcount: 1,
            ancount: 0,
            nscount: 0,
            arcount: 0,
        }
    }

    pub fn new_response(id: u16, questions: u16, answers: u16) -> Self {
        Self {
            id,
            flags: 0x8180, // QR=1 (response), RD=1, RA=1 (recursion available)
            qdcount: questions,
            ancount: answers,
            nscount: 0,
            arcount: 0,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12);
        bytes.extend_from_slice(&self.id.to_be_bytes());
        bytes.extend_from_slice(&self.flags.to_be_bytes());
        bytes.extend_from_slice(&self.qdcount.to_be_bytes());
        bytes.extend_from_slice(&self.ancount.to_be_bytes());
        bytes.extend_from_slice(&self.nscount.to_be_bytes());
        bytes.extend_from_slice(&self.arcount.to_be_bytes());
        bytes
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 12 {
            return None;
        }
        
        Some(Self {
            id: u16::from_be_bytes([data[0], data[1]]),
            flags: u16::from_be_bytes([data[2], data[3]]),
            qdcount: u16::from_be_bytes([data[4], data[5]]),
            ancount: u16::from_be_bytes([data[6], data[7]]),
            nscount: u16::from_be_bytes([data[8], data[9]]),
            arcount: u16::from_be_bytes([data[10], data[11]]),
        })
    }
}

impl DnsQuestion {
    pub fn new(qname: String, qtype: u16) -> Self {
        Self {
            qname,
            qtype,
            qclass: 1, // IN (Internet)
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // Encoder le nom de domaine
        for label in self.qname.split('.') {
            if !label.is_empty() {
                bytes.push(label.len() as u8);
                bytes.extend_from_slice(label.as_bytes());
            }
        }
        bytes.push(0); // Terminateur
        
        bytes.extend_from_slice(&self.qtype.to_be_bytes());
        bytes.extend_from_slice(&self.qclass.to_be_bytes());
        bytes
    }

    pub fn from_bytes(data: &[u8], offset: &mut usize) -> Option<Self> {
        let qname = decode_domain_name(data, offset)?;
        
        if *offset + 4 > data.len() {
            return None;
        }
        
        let qtype = u16::from_be_bytes([data[*offset], data[*offset + 1]]);
        let qclass = u16::from_be_bytes([data[*offset + 2], data[*offset + 3]]);
        *offset += 4;
        
        Some(Self { qname, qtype, qclass })
    }
}

impl DnsResourceRecord {
    pub fn new_a_record(name: String, ip: Ipv4Addr, ttl: u32) -> Self {
        Self {
            name,
            rtype: 1, // A record
            rclass: 1, // IN
            ttl,
            rdlength: 4,
            rdata: ip.octets().to_vec(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // Encoder le nom
        for label in self.name.split('.') {
            if !label.is_empty() {
                bytes.push(label.len() as u8);
                bytes.extend_from_slice(label.as_bytes());
            }
        }
        bytes.push(0);
        
        bytes.extend_from_slice(&self.rtype.to_be_bytes());
        bytes.extend_from_slice(&self.rclass.to_be_bytes());
        bytes.extend_from_slice(&self.ttl.to_be_bytes());
        bytes.extend_from_slice(&self.rdlength.to_be_bytes());
        bytes.extend_from_slice(&self.rdata);
        
        bytes
    }

    pub fn from_bytes(data: &[u8], offset: &mut usize) -> Option<Self> {
        let name = decode_domain_name(data, offset)?;
        
        if *offset + 10 > data.len() {
            return None;
        }
        
        let rtype = u16::from_be_bytes([data[*offset], data[*offset + 1]]);
        let rclass = u16::from_be_bytes([data[*offset + 2], data[*offset + 3]]);
        let ttl = u32::from_be_bytes([
            data[*offset + 4], data[*offset + 5], 
            data[*offset + 6], data[*offset + 7]
        ]);
        let rdlength = u16::from_be_bytes([data[*offset + 8], data[*offset + 9]]);
        *offset += 10;
        
        if *offset + rdlength as usize > data.len() {
            return None;
        }
        
        let rdata = data[*offset..*offset + rdlength as usize].to_vec();
        *offset += rdlength as usize;
        
        Some(Self {
            name, rtype, rclass, ttl, rdlength, rdata
        })
    }
}

// Fonction utilitaire pour décoder les noms de domaine DNS
fn decode_domain_name(data: &[u8], offset: &mut usize) -> Option<String> {
    let mut labels = Vec::new();
    let mut pos = *offset;
    let mut jumped = false;
    
    loop {
        if pos >= data.len() {
            return None;
        }
        
        let len = data[pos];
        
        if len == 0 {
            pos += 1;
            if !jumped {
                *offset = pos;
            }
            break;
        }
        
        if len & 0xC0 == 0xC0 {
            // Pointeur de compression
            if pos + 1 >= data.len() {
                return None;
            }
            if !jumped {
                *offset = pos + 2;
            }
            pos = ((len & 0x3F) as usize) << 8 | data[pos + 1] as usize;
            jumped = true;
            continue;
        }
        
        pos += 1;
        if pos + len as usize > data.len() {
            return None;
        }
        
        let label = String::from_utf8(data[pos..pos + len as usize].to_vec()).ok()?;
        labels.push(label);
        pos += len as usize;
    }
    
    Some(labels.join("."))
}

impl DnsMessage {
    pub fn new_query(id: u16, domain: &str) -> Self {
        Self {
            header: DnsHeader::new_query(id),
            questions: vec![DnsQuestion::new(domain.to_string(), 1)], // Type A
            answers: Vec::new(),
            authority: Vec::new(),
            additional: Vec::new(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        bytes.extend_from_slice(&self.header.to_bytes());
        
        for question in &self.questions {
            bytes.extend_from_slice(&question.to_bytes());
        }
        
        for answer in &self.answers {
            bytes.extend_from_slice(&answer.to_bytes());
        }
        
        bytes
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let header = DnsHeader::from_bytes(data)?;
        let mut offset = 12;
        
        let mut questions = Vec::new();
        for _ in 0..header.qdcount {
            let question = DnsQuestion::from_bytes(data, &mut offset)?;
            questions.push(question);
        }
        
        let mut answers = Vec::new();
        for _ in 0..header.ancount {
            let answer = DnsResourceRecord::from_bytes(data, &mut offset)?;
            answers.push(answer);
        }
        
        Some(Self {
            header,
            questions,
            answers,
            authority: Vec::new(),
            additional: Vec::new(),
        })
    }
}

pub struct DnsClient {
    socket: UdpSocket,
    server_addr: SocketAddr,
}

impl DnsClient {
    pub async fn new(server_addr: SocketAddr) -> IoResult<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { socket, server_addr })
    }

    pub async fn resolve(&self, domain: &str) -> IoResult<Option<Ipv4Addr>> {
        let query_id = rand::random_u16();
        let query = DnsMessage::new_query(query_id, domain);
        let query_bytes = query.to_bytes();

        // Envoyer la requête
        self.socket.send_to(&query_bytes, &self.server_addr).await?;
        
        // Recevoir la réponse
        let mut buf = [0u8; 512];
        let (len, _) = self.socket.recv_from(&mut buf).await?;
        
        if let Some(response) = DnsMessage::from_bytes(&buf[..len]) {
            if response.header.id == query_id && !response.answers.is_empty() {
                // Extraire l'adresse IP de la première réponse de type A
                for answer in &response.answers {
                    if answer.rtype == 1 && answer.rdata.len() == 4 {
                        let ip = Ipv4Addr::new(
                            answer.rdata[0],
                            answer.rdata[1], 
                            answer.rdata[2],
                            answer.rdata[3]
                        );
                        return Ok(Some(ip));
                    }
                }
            }
        }
        
        Ok(None)
    }
}

pub struct DnsServer {
    socket: UdpSocket,
    records: HashMap<String, Ipv4Addr>,
}

impl DnsServer {
    pub async fn new(bind_addr: SocketAddr) -> IoResult<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let mut records = HashMap::new();
        
        // Ajouter quelques enregistrements prédéfinis
        records.insert("example.com".to_string(), Ipv4Addr::new(93, 184, 216, 34));
        records.insert("test.local".to_string(), Ipv4Addr::new(192, 168, 1, 100));
        records.insert("myserver.local".to_string(), Ipv4Addr::new(10, 0, 0, 1));
        records.insert("localhost".to_string(), Ipv4Addr::new(127, 0, 0, 1));
        
        Ok(Self { socket, records })
    }

    pub fn add_record(&mut self, domain: String, ip: Ipv4Addr) {
        self.records.insert(domain, ip);
    }

    pub async fn run(&self) -> IoResult<()> {
        println!("Serveur DNS démarré sur {}", self.socket.local_addr()?);
        println!("Domaines configurés:");
        for (domain, ip) in &self.records {
            println!("  {} -> {}", domain, ip);
        }
        
        let mut buf = [0u8; 512];
        
        loop {
            let (len, src) = self.socket.recv_from(&mut buf).await?;
            
            if let Some(query) = DnsMessage::from_bytes(&buf[..len]) {
                let response = self.handle_query(query);
                let response_bytes = response.to_bytes();
                
                self.socket.send_to(&response_bytes, &src).await?;
                
                if let Some(question) = response.questions.first() {
                    let status = if response.answers.is_empty() { "NXDOMAIN" } else { "RESOLVED" };
                    println!("Query from {}: {} -> {}", src, question.qname, status);
                }
            }
        }
    }

    fn handle_query(&self, query: DnsMessage) -> DnsMessage {
        let mut response = DnsMessage {
            header: DnsHeader::new_response(query.header.id, 1, 0),
            questions: query.questions.clone(),
            answers: Vec::new(),
            authority: Vec::new(),
            additional: Vec::new(),
        };

        // Traiter la première question (DNS simple)
        if let Some(question) = query.questions.first() {
            if question.qtype == 1 { // Type A
                if let Some(&ip) = self.records.get(&question.qname) {
                    let answer = DnsResourceRecord::new_a_record(
                        question.qname.clone(),
                        ip,
                        300 // TTL de 5 minutes
                    );
                    response.answers.push(answer);
                    response.header.ancount = 1;
                }
            }
        }

        response
    }
}

#[tokio::main]
async fn main() -> IoResult<()> {
    println!("Client et Serveur DNS Simple\n");
    
    // Démarrer le serveur DNS en arrière-plan
    let server_addr = SocketAddr::from(([127, 0, 0, 1], 8053));
    let server = DnsServer::new(server_addr).await?;
    
    tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("Erreur serveur DNS: {}", e);
        }
    });
    
    // Attendre un peu que le serveur démarre
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Tester le client DNS
    println!("\nTest du Client DNS");
    let client = DnsClient::new(server_addr).await?;
    
    let test_domains = vec![
        "example.com",
        "test.local", 
        "localhost",
        "unknown.domain"
    ];
    
    for domain in test_domains {
        match client.resolve(domain).await? {
            Some(ip) => println!("{} résolu vers {}", domain, ip),
            None => println!("{} non trouvé", domain),
        }
    }
    
    println!("\nTest avec serveur DNS Google (8.8.8.8)");
    let google_dns = SocketAddr::from(([8, 8, 8, 8], 53));
    let google_client = DnsClient::new(google_dns).await?;
    
    match google_client.resolve("google.com").await? {
        Some(ip) => println!("google.com résolu vers {} (via 8.8.8.8)", ip),
        None => println!("google.com non résolu"),
    }
    
    println!("\nAppuyez sur Ctrl+C pour arrêter le serveur...");
    
    // Garder le programme en vie
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

// Module utilitaire pour générer des nombres aléatoires simples
mod rand {
    use std::convert::TryFrom;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(1);

    pub fn random_u16() -> u16 {
    let val = COUNTER.fetch_add(1, Ordering::Relaxed);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    (val.wrapping_mul(31).wrapping_add(time) & 0xFFFF) as u16
}
}
