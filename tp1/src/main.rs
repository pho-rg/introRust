use std::io;

fn main() {
    let nom_compte = "Kevin";
    let mut solde: f64 = 1500.50;
    
    let comptes = ["Kevin - 1500.50€", "Marie - 850.75€", "Pierre - 2200.00€"];
    
    println!("Banque TP1");
    println!("Compte de : {}", nom_compte);
    
    loop {
        let options = ["Afficher solde", "Retrait", "Liste comptes", "Quitter"];
        
        println!("\nMenu:");
        for (i, option) in options.iter().enumerate() {
            println!("{}. {}", i + 1, option);
        }
        
        println!("Veuillez saisir un numéro de votre choix:");
        
        let mut choix = String::new();
        io::stdin().read_line(&mut choix);
        
        let choix: usize = match choix.trim().parse() {
            Ok(num) => num,
            Err(_) => {
                println!("Veuillez saisir un numero valide");
                continue;
            }
        };
                
        if choix == 1 {
            afficher_solde(nom_compte, solde);
        } else if choix == 2 {
            solde = faire_retrait(solde);
        } else if choix == 3 {
            lister_comptes(&comptes);
        } else if choix == 4 {
            println!("Quitter");
            break;
        }
    }
}

fn afficher_solde(nom: &str, solde: f64) {
    println!("Solde du compte");
    println!("Compte de : {}", nom);
    println!("Solde actuel : {:.2}€", solde);
}

fn faire_retrait(solde_actuel: f64) -> f64 {
    println!("Retrait");
    println!("Solde actuel : {:.2}€", solde_actuel);
    println!("Montant à retirer :");
    
    let mut montant = String::new();
    io::stdin().read_line(&mut montant);
    
    let montant: f64 = match montant.trim().parse() {
        Ok(m) => m,
        Err(_) => {
            println!("Montant invalide");
            return solde_actuel;
        }
    };
    
    if montant <= 0.0 {
        println!("Le montant doit être positif !");
        return solde_actuel;
    }
    
    if montant > solde_actuel {
        println!("Solde insuffisant");
        return solde_actuel;
    }
    
    let nouveau_solde = solde_actuel - montant;
    println!("Retrait de {:.2}€ effectué", montant);
    println!("Nouveau solde : {:.2}€", nouveau_solde);
    
    nouveau_solde
}

fn lister_comptes(comptes: &[&str]) {
    println!("Liste des comptes");
    
    for (i, compte) in comptes.iter().enumerate() {
        println!("{}. {}", i + 1, compte);
    }
}