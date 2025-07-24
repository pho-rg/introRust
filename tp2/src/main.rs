use std::fs::{File, OpenOptions, remove_file, metadata};
use std::io::{self, Write, Read, BufRead, BufReader, stdin, stdout};
use std::path::Path;

#[derive(Debug)]
struct FileManager {
    current_file: Option<String>,
}

impl FileManager {
    fn new() -> Self {
        FileManager {
            current_file: None,
        }
    }

    fn display_menu(&self) {
        println!("\n=== GESTIONNAIRE DE FICHIERS RUST ===");
        println!("1. Créer un nouveau fichier");
        println!("2. Lire un fichier");
        println!("3. Écrire dans un fichier");
        println!("4. Modifier un fichier");
        println!("5. Supprimer un fichier");
        println!("6. Lister les fichiers du répertoire");
        println!("7. Informations sur le fichier courant");
        println!("0. Quitter");
        
        if let Some(ref file) = self.current_file {
            println!("Fichier courant: {}", file);
        }
        
        print!("\nVotre choix: ");
        stdout().flush().unwrap();
    }

    fn create_file(&mut self) {
        let filename = self.get_filename("Nom du nouveau fichier à créer");
        
        // Vérifier si le fichier existe déjà
        if Path::new(&filename).exists() {
            println!("Le fichier {} existe déjà!", filename);
            println!("Voulez-vous l'écraser ? (oui/non)");
            let confirmation = self.get_input("");
            
            match confirmation.trim().to_lowercase().as_str() {
                "oui" | "o" | "yes" | "y" => {
                    println!("Le fichier existant sera écrasé.");
                }
                _ => {
                    println!("Création annulée.");
                    return;
                }
            }
        }

        match File::create(&filename) {
            Ok(mut file) => {
                println!("Fichier {} créé avec succès!", filename);
                println!("Voulez-vous ajouter du contenu maintenant ? (oui/non)");
                let add_content = self.get_input("");
                
                match add_content.trim().to_lowercase().as_str() {
                    "oui" | "o" | "yes" | "y" => {
                        println!("Entrez le contenu (tapez 'EOF' sur une ligne vide pour terminer):");
                        
                        let mut content = String::new();
                        loop {
                            let line = self.get_input("");
                            if line.trim() == "EOF" {
                                break;
                            }
                            content.push_str(&line);
                            content.push('\n');
                        }

                        match file.write_all(content.as_bytes()) {
                            Ok(_) => {
                                println!("Contenu ajouté avec succès!");
                                self.current_file = Some(filename.clone());
                            }
                            Err(e) => println!("Erreur lors de l'écriture du contenu: {}", e),
                        }
                    }
                    _ => {
                        self.current_file = Some(filename.clone());
                    }
                }
            }
            Err(e) => println!("Erreur lors de la création du fichier: {}", e),
        }
    }

    fn read_file(&mut self) {
        let filename = self.get_filename("Nom du fichier à lire");
        
        match File::open(&filename) {
            Ok(file) => {
                let reader = BufReader::new(file);
                println!("\n--- Contenu de {} ---", filename);
                
                let mut line_number = 1;
                for line in reader.lines() {
                    match line {
                        Ok(content) => println!("{:3}: {}", line_number, content),
                        Err(e) => {
                            println!("Erreur lors de la lecture de la ligne {}: {}", line_number, e);
                            break;
                        }
                    }
                    line_number += 1;
                }
                
                self.current_file = Some(filename.clone());
            }
            Err(e) => println!("Erreur lors de l'ouverture du fichier: {}", e),
        }
    }

    fn write_file(&mut self) {
        let filename = self.get_filename("Nom du fichier à écrire");
        
        println!("Mode d'écriture:");
        println!("1. Écraser le contenu existant");
        println!("2. Ajouter à la fin du fichier");
        
        let mode = self.get_input("Votre choix (1-2)");
        
        let file_result = match mode.trim() {
            "1" => File::create(&filename),
            "2" => OpenOptions::new().create(true).append(true).open(&filename),
            _ => {
                println!("Choix invalide!");
                return;
            }
        };

        match file_result {
            Ok(mut file) => {
                println!("Entrez le contenu (tapez 'EOF' sur une ligne vide pour terminer):");
                
                let mut content = String::new();
                loop {
                    let line = self.get_input("");
                    if line.trim() == "EOF" {
                        break;
                    }
                    content.push_str(&line);
                    content.push('\n');
                }

                match file.write_all(content.as_bytes()) {
                    Ok(_) => {
                        println!("Contenu écrit avec succès dans {}", filename);
                        self.current_file = Some(filename.clone());
                    }
                    Err(e) => println!("Erreur lors de l'écriture: {}", e),
                }
            }
            Err(e) => println!("Erreur lors de l'ouverture du fichier: {}", e),
        }
    }

    fn modify_file(&mut self) {
        let filename = self.get_filename("Nom du fichier à modifier");
        
        // Lire le contenu existant
        let mut content = String::new();
        match File::open(&filename) {
            Ok(mut file) => {
                if let Err(e) = file.read_to_string(&mut content) {
                    println!("Erreur lors de la lecture: {}", e);
                    return;
                }
            }
            Err(e) => {
                println!("Erreur lors de l'ouverture: {}", e);
                return;
            }
        }

        println!("\n--- Contenu actuel ---");
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            println!("{:3}: {}", i + 1, line);
        }

        println!("\nOptions de modification:");
        println!("1. Remplacer une ligne spécifique");
        println!("2. Ajouter une ligne à une position");
        println!("3. Supprimer une ligne");
        
        let choice = self.get_input("Votre choix (1-3)");
        
        let mut new_lines = lines.iter().map(|&s| s.to_string()).collect::<Vec<String>>();
        
        match choice.trim() {
            "1" => {
                let line_num = self.get_input("Numéro de ligne à remplacer");
                if let Ok(num) = line_num.trim().parse::<usize>() {
                    if num > 0 && num <= new_lines.len() {
                        let new_content = self.get_input("Nouveau contenu");
                        new_lines[num - 1] = new_content;
                    } else {
                        println!("Numéro de ligne invalide!");
                        return;
                    }
                }
            }
            "2" => {
                let line_num = self.get_input("Position d'insertion (numéro de ligne)");
                if let Ok(num) = line_num.trim().parse::<usize>() {
                    if num > 0 && num <= new_lines.len() + 1 {
                        let new_content = self.get_input("Contenu à ajouter");
                        new_lines.insert(num - 1, new_content);
                    } else {
                        println!("Position invalide!");
                        return;
                    }
                }
            }
            "3" => {
                let line_num = self.get_input("Numéro de ligne à supprimer");
                if let Ok(num) = line_num.trim().parse::<usize>() {
                    if num > 0 && num <= new_lines.len() {
                        new_lines.remove(num - 1);
                    } else {
                        println!("Numéro de ligne invalide!");
                        return;
                    }
                }
            }
            _ => {
                println!("Choix invalide!");
                return;
            }
        }

        // Écrire le contenu modifié
        match File::create(&filename) {
            Ok(mut file) => {
                let new_content = new_lines.join("\n") + "\n";
                if let Err(e) = file.write_all(new_content.as_bytes()) {
                    println!("Erreur lors de l'écriture: {}", e);
                } else {
                    println!("Fichier modifié avec succès!");
                    self.current_file = Some(filename.clone());
                }
            }
            Err(e) => println!("Erreur lors de la création du fichier: {}", e),
        }
    }

    fn delete_file(&mut self) {
        let filename = self.get_filename("Nom du fichier à supprimer");
        
        if !Path::new(&filename).exists() {
            println!("Le fichier {} n'existe pas!", filename);
            return;
        }

        println!("Êtes-vous sûr de vouloir supprimer {} ? (oui/non)", filename);
        let confirmation = self.get_input("");
        
        match confirmation.trim().to_lowercase().as_str() {
            "oui" | "o" | "yes" | "y" => {
                match remove_file(&filename) {
                    Ok(_) => {
                        println!("Fichier {} supprimé avec succès!", filename);
                        if let Some(ref current) = self.current_file {
                            if current == &filename {
                                self.current_file = None;
                            }
                        }
                    }
                    Err(e) => println!("Erreur lors de la suppression: {}", e),
                }
            }
            _ => println!("Suppression annulée."),
        }
    }

    fn list_files(&self) {
        println!("\n--- Fichiers du répertoire courant ---");
        
        match std::fs::read_dir(".") {
            Ok(entries) => {
                let mut files = Vec::new();
                let mut dirs = Vec::new();
                
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        let name = path.file_name().unwrap().to_string_lossy().to_string();
                        
                        if path.is_dir() {
                            dirs.push(name);
                        } else {
                            files.push(name);
                        }
                    }
                }
                
                dirs.sort();
                files.sort();
                
                if !dirs.is_empty() {
                    println!("\nRépertoires:");
                    for dir in dirs {
                        println!("  [DIR]  {}", dir);
                    }
                }
                
                if !files.is_empty() {
                    println!("\nFichiers:");
                    for file in files {
                        println!("  [FILE] {}", file);
                    }
                }
            }
            Err(e) => println!("Erreur lors de la lecture du répertoire: {}", e),
        }
    }

    fn show_file_info(&self) {
        let filename = match &self.current_file {
            Some(file) => file.clone(),
            None => self.get_filename("Nom du fichier pour les informations"),
        };

        match metadata(&filename) {
            Ok(meta) => {
                println!("\n--- Informations sur {} ---", filename);
                println!("Taille: {} octets", meta.len());
                println!("Lecture seule: {}", meta.permissions().readonly());
                println!("Type: {}", if meta.is_dir() { "Répertoire" } else { "Fichier" });
                
                if let Ok(modified) = meta.modified() {
                    println!("Dernière modification: {:?}", modified);
                }
            }
            Err(e) => println!("Erreur lors de la récupération des métadonnées: {}", e),
        }
    }

    fn get_filename(&self, prompt: &str) -> String {
        self.get_input(prompt)
    }

    fn get_input(&self, prompt: &str) -> String {
        if !prompt.is_empty() {
            print!("{}: ", prompt);
            stdout().flush().unwrap();
        }
        
        let mut input = String::new();
        stdin().read_line(&mut input).expect("Erreur lors de la lecture");
        input.trim().to_string()
    }

    fn run(&mut self) {
        loop {
            self.display_menu();
            
            let mut input = String::new();
            if stdin().read_line(&mut input).is_err() {
                println!("Erreur lors de la lecture de l'entrée.");
                continue;
            }

            match input.trim() {
                "1" => self.create_file(),
                "2" => self.read_file(),
                "3" => self.write_file(),
                "4" => self.modify_file(),
                "5" => self.delete_file(),
                "6" => self.list_files(),
                "7" => self.show_file_info(),
                "0" => {
                    println!("Au revoir!");
                    break;
                }
                _ => println!("Choix invalide! Veuillez choisir entre 0 et 7."),
            }

            // Pause pour permettre à l'utilisateur de lire les résultats
            println!("\nAppuyez sur Entrée pour continuer...");
            let mut pause = String::new();
            stdin().read_line(&mut pause).unwrap();
        }
    }
}

fn main() {
    let mut file_manager = FileManager::new();
    file_manager.run();
}