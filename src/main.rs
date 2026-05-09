mod store;

use store::engine::RiverStore;

fn main() {
    let mut river = RiverStore::new();

    river.set("name".to_string(), "Water".to_string());

    println!("{:?}", river.get("name"));

    river.delete("name");

    println!("{:?}", river.get("name"));
}