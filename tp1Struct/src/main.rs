use std::io;

struct Compte {
    nom: String,
    solde: f64,
}

fn main() {
    let mut compte_principal = Compte {
        nom: String::from("Kevin"),
        solde: 1500.50,
    };
    
    let autres_comptes = [
        Compte {
            nom: String::from("Marie"),
            solde: 850.75,
        },
        Compte {
            nom: String::from("Pierre"), 
            solde: 2200.00,
        },
    ];
    
    println!("Banque TP1");
    println!("Compte de : {}", compte_principal.nom);
    
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
            afficher_solde(&compte_principal);
        } else if choix == 2 {
            faire_retrait(&mut compte_principal);
        } else if choix == 3 {
            lister_comptes(&compte_principal, &autres_comptes);
        } else if choix == 4 {
            println!("Quitter");
            break;
        }
    }
}

// Fonction pour afficher le solde d'un compte
fn afficher_solde(compte: &Compte) {
    println!("Solde du compte");
    println!("Compte de : {}", compte.nom);
    println!("Solde actuel : {:.2}€", compte.solde);
}

// Fonction pour faire un retrait sur un compte
fn faire_retrait(compte: &mut Compte) {
    println!("Retrait");
    println!("Solde actuel : {:.2}€", compte.solde);
    println!("Montant à retirer :");
    
    let mut montant = String::new();
    io::stdin().read_line(&mut montant);
    
    let montant: f64 = match montant.trim().parse() {
        Ok(m) => m,
        Err(_) => {
            println!("Montant invalide");
            return;
        }
    };
    
    if montant <= 0.0 {
        println!("Le montant doit être positif !");
        return;
    }
    
    if montant > compte.solde {
        println!("Solde insuffisant");
        return;
    }
    
    compte.solde = compte.solde - montant;
    println!("Retrait de {:.2}€ effectué", montant);
    println!("Nouveau solde : {:.2}€", compte.solde);
}

// Fonction pour lister tous les comptes
fn lister_comptes(compte_principal: &Compte, autres_comptes: &[Compte]) {
    println!("Liste des comptes");
    
    // Afficher le compte principal
    println!("1. {} - {:.2}€", compte_principal.nom, compte_principal.solde);
    
    // Afficher les autres comptes
    for (i, compte) in autres_comptes.iter().enumerate() {
        println!("{}. {} - {:.2}€", i + 2, compte.nom, compte.solde);
    }
}