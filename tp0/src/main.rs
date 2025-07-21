fn main() {
    let nom = "ph";
    let age:u32 = 30; // non signé sur 32 bits
    let age_papa = 70;
    let temperature:f32 = 32.5;

    println!("Hello, world!");
    println!("{} a {} ans", nom, age);
    println!("Papa a {} ans", age_papa);
    println!("Il fait {}°C ajd", temperature);

    let resultat = addition(12, 3);
    println!("la somme est {}", resultat);

    for i in 1..=10{
        println!("i vaut {}", i);
    }
}

fn addition(n1:i32, n2:i32) -> i32 {
    n1 + n2
}